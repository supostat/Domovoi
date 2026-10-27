#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DuplicateKey {
    pub group: usize,
    pub member: Option<usize>,
}

impl DuplicateKey {
    pub fn parse(key: &str) -> Self {
        let mut parts = key
            .split('.')
            .map(|part| part.parse::<usize>().expect("duplicate keys are numeric"));
        Self {
            group: parts.next().expect("a key names a group"),
            member: parts.next(),
        }
    }

    pub fn into_pair(self) -> (usize, Option<usize>) {
        (self.group, self.member)
    }

    pub fn text(self) -> String {
        match self.member {
            None => format!("{}", self.group),
            Some(member) => format!("{}.{member}", self.group),
        }
    }
}
