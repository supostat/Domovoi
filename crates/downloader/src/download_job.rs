use std::fs;
use std::ops::RangeInclusive;
use std::sync::Arc;

use domovoi_index::ModelEvent;
use objc2::rc::{Retained, autoreleasepool};
use objc2_foundation::NSURLSessionDataTask;

use crate::catalog_entry::CatalogEntry;
use crate::download_control::DownloadControl;
use crate::download_outcome::DownloadOutcome;
use crate::download_progress::DownloadProgress;
use crate::halt::Halt;
use crate::install_error::InstallError;
use crate::model_source::ModelSource;
use crate::model_store::ModelStore;
use crate::partial_file::PartialFile;
use crate::response_head::ResponseHead;
use crate::sha256_digest::Sha256Digest;
use crate::signed_link::SignedLink;
use crate::stop_request::StopRequest;
use crate::transfer_event::TransferEvent;
use crate::transfer_session::TransferSession;
use crate::transfer_step::TransferStep;

const OK: u16 = 200;
const PARTIAL_CONTENT: u16 = 206;
const FORBIDDEN: u16 = 403;
const REDIRECTION: RangeInclusive<u16> = 300..=399;
const MAX_REFUSED_LINKS: u32 = 3;
const MAX_INTERRUPTIONS: u32 = 3;
const NO_REDIRECT: &str = "the source finished without redirecting to the file";

pub(crate) struct DownloadJob {
    pub(crate) entry: CatalogEntry,
    pub(crate) source: ModelSource,
    pub(crate) store: ModelStore,
    pub(crate) control: Arc<DownloadControl>,
    pub(crate) on_progress: Box<dyn FnMut(DownloadProgress) + Send>,
}

impl DownloadJob {
    pub(crate) fn run(mut self) -> DownloadOutcome {
        match autoreleasepool(|_| self.transfer()) {
            Ok(event) => DownloadOutcome::Installed(event),
            Err(Halt::Stopped(StopRequest::Pause)) => DownloadOutcome::Paused,
            Err(Halt::Stopped(StopRequest::Cancel)) => {
                let partial = self.store.partial_path(&self.entry);
                if !partial.exists() {
                    return DownloadOutcome::Cancelled;
                }
                match self.store.move_to_trash(&partial) {
                    Ok(()) => DownloadOutcome::Cancelled,
                    Err(error) => DownloadOutcome::Failed(error),
                }
            }
            Err(Halt::Failed(error)) => DownloadOutcome::Failed(error),
        }
    }

    fn transfer(&mut self) -> Result<ModelEvent, Halt> {
        let partial_path = self.store.partial_path(&self.entry);
        let already_received = fs::metadata(&partial_path).map_or(0, |metadata| metadata.len());
        self.store
            .ensure_space(&self.entry, already_received.min(self.entry.size))?;
        self.check_stop()?;
        let mut partial = PartialFile::open(&partial_path, self.entry.size)?;
        self.report(&partial);
        let session = TransferSession::new();
        let mut link = self.resolve(&session)?;
        let mut refused_links = 0;
        let mut interruptions = 0;
        while partial.received() < self.entry.size {
            self.check_stop()?;
            if link.has_expired() {
                link = self.resolve(&session)?;
            }
            let before = partial.received();
            match self.fetch(&session, &link, &mut partial)? {
                TransferStep::Restart => {}
                TransferStep::Complete if partial.received() > before => {}
                TransferStep::Complete => {
                    interruptions += 1;
                    if interruptions > MAX_INTERRUPTIONS {
                        return Err(InstallError::Network("the source keeps ending the file early".to_owned()).into());
                    }
                }
                TransferStep::Forbidden => {
                    refused_links += 1;
                    if refused_links > MAX_REFUSED_LINKS {
                        return Err(InstallError::AccessDenied.into());
                    }
                    link = self.resolve(&session)?;
                }
                TransferStep::Interrupted(description) => {
                    self.check_stop()?;
                    interruptions += 1;
                    if interruptions > MAX_INTERRUPTIONS {
                        return Err(InstallError::Network(description).into());
                    }
                }
            }
        }
        partial.verify(self.entry.sha256)?;
        Ok(self.store.install_partial(&self.entry)?)
    }

    fn resolve(&self, session: &TransferSession) -> Result<SignedLink, Halt> {
        let address = self.source.resolve_url(&self.entry);
        let task = session.start(&address, &[])?;
        self.control.attach(&task);
        let link = self.await_redirect(session, &task, &address);
        self.control.detach();
        if link.is_err() {
            task.cancel();
        }
        link
    }

    fn await_redirect(
        &self,
        session: &TransferSession,
        task: &Retained<NSURLSessionDataTask>,
        address: &str,
    ) -> Result<SignedLink, Halt> {
        let mut link = None;
        loop {
            match session.next_event(task) {
                TransferEvent::Response(head) if REDIRECTION.contains(&head.status) => {
                    self.check_resolution(&head)?;
                    let location = head.location.ok_or(InstallError::UnexpectedStatus(head.status))?;
                    link = Some(SignedLink::new(self.absolute(location)));
                }
                TransferEvent::Response(head) if head.status == OK || head.status == PARTIAL_CONTENT => {
                    self.check_resolution(&head)?;
                    task.cancel();
                    return Ok(SignedLink::new(address.to_owned()));
                }
                TransferEvent::Response(head) => return Err(InstallError::UnexpectedStatus(head.status).into()),
                TransferEvent::Data(_) => {}
                TransferEvent::Finished(failure) => {
                    self.check_stop()?;
                    return match (link, failure) {
                        (Some(link), None) => Ok(link),
                        (_, failure) => {
                            Err(InstallError::Network(failure.unwrap_or_else(|| NO_REDIRECT.to_owned())).into())
                        }
                    };
                }
            }
        }
    }

    fn check_resolution(&self, head: &ResponseHead) -> Result<(), InstallError> {
        if let Some(served) = &head.repo_commit
            && *served != self.entry.revision
        {
            return Err(InstallError::RevisionMismatch {
                expected: self.entry.revision.clone(),
                served: served.clone(),
            });
        }
        if let Some(served) = head
            .linked_etag
            .as_deref()
            .and_then(|etag| etag.trim_matches('"').parse::<Sha256Digest>().ok())
            && served != self.entry.sha256
        {
            return Err(InstallError::DigestMismatch {
                expected: self.entry.sha256,
                received: served,
            });
        }
        if let Some(served) = head.linked_size.as_deref().and_then(|size| size.parse::<u64>().ok())
            && served != self.entry.size
        {
            return Err(InstallError::SizeMismatch {
                expected: self.entry.size,
                received: served,
            });
        }
        Ok(())
    }

    fn absolute(&self, location: String) -> String {
        if location.starts_with('/') {
            format!("{}{location}", self.source.base())
        } else {
            location
        }
    }

    fn fetch(
        &mut self,
        session: &TransferSession,
        link: &SignedLink,
        partial: &mut PartialFile,
    ) -> Result<TransferStep, Halt> {
        let mut headers = Vec::new();
        if partial.received() > 0 {
            headers.push(("Range", format!("bytes={}-", partial.received())));
            if let Some(validator) = partial.validator() {
                headers.push(("If-Range", validator.to_owned()));
            }
        }
        let task = session.start(&link.url, &headers)?;
        self.control.attach(&task);
        let step = self.stream(session, &task, partial);
        self.control.detach();
        if !matches!(step, Ok(TransferStep::Complete | TransferStep::Interrupted(_))) {
            task.cancel();
        }
        step
    }

    fn stream(
        &mut self,
        session: &TransferSession,
        task: &Retained<NSURLSessionDataTask>,
        partial: &mut PartialFile,
    ) -> Result<TransferStep, Halt> {
        loop {
            match session.next_event(task) {
                TransferEvent::Response(head) => match head.status {
                    PARTIAL_CONTENT => {
                        if !self.continues(&head, partial) {
                            partial.restart()?;
                            return Ok(TransferStep::Restart);
                        }
                        if let Some(etag) = &head.etag {
                            partial.remember_validator(etag)?;
                        }
                    }
                    OK => {
                        partial.restart()?;
                        if let Some(etag) = &head.etag {
                            partial.remember_validator(etag)?;
                        }
                    }
                    FORBIDDEN => return Ok(TransferStep::Forbidden),
                    status => return Err(InstallError::UnexpectedStatus(status).into()),
                },
                TransferEvent::Data(bytes) => {
                    partial.append(&bytes, self.entry.size)?;
                    self.report(partial);
                }
                TransferEvent::Finished(None) => return Ok(TransferStep::Complete),
                TransferEvent::Finished(Some(description)) => return Ok(TransferStep::Interrupted(description)),
            }
        }
    }

    fn continues(&self, head: &ResponseHead, partial: &PartialFile) -> bool {
        let expected_range = format!(
            "bytes {}-{}/{}",
            partial.received(),
            self.entry.size - 1,
            self.entry.size
        );
        let same_validator = partial
            .validator()
            .is_none_or(|validator| head.etag.as_deref() == Some(validator));
        head.content_range.as_deref() == Some(expected_range.as_str()) && same_validator
    }

    fn report(&mut self, partial: &PartialFile) {
        (self.on_progress)(DownloadProgress {
            received: partial.received(),
            total: self.entry.size,
        });
    }

    fn check_stop(&self) -> Result<(), Halt> {
        self.control
            .requested()
            .map_or(Ok(()), |request| Err(Halt::Stopped(request)))
    }
}
