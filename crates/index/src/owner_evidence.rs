#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum OwnerEvidence {
    BundleIdName,
    ContainerMetadata,
    AppGroup,
    TeamPrefix,
    LaunchdAssociation,
    NameOnly,
}
