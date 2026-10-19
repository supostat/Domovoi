use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct NameId(u32);

impl NameId {
    pub(crate) const NONE: Self = Self(u32::MAX);
}

#[derive(Clone, Debug, Default, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub(crate) struct NameTable {
    bytes: Vec<u8>,
    ends: Vec<u32>,
}

impl NameTable {
    pub(crate) fn get(&self, id: NameId) -> &OsStr {
        let index = id.0 as usize;
        let start = if index == 0 { 0 } else { self.ends[index - 1] as usize };
        OsStr::from_bytes(&self.bytes[start..self.ends[index] as usize])
    }
}

#[derive(Debug, Default)]
pub(crate) struct NameInterner {
    table: NameTable,
    known: HashMap<Vec<u8>, NameId>,
}

impl NameInterner {
    pub(crate) fn appending(table: NameTable) -> Self {
        Self {
            table,
            known: HashMap::new(),
        }
    }

    pub(crate) fn intern(&mut self, name: &OsStr) -> NameId {
        if let Some(id) = self.known.get(name.as_bytes()) {
            return *id;
        }
        self.table.bytes.extend_from_slice(name.as_bytes());
        self.table
            .ends
            .push(u32::try_from(self.table.bytes.len()).expect("interned names fit in 4 GiB"));
        let id = NameId(u32::try_from(self.table.ends.len() - 1).expect("name count fits in u32"));
        self.known.insert(name.as_bytes().to_vec(), id);
        id
    }

    pub(crate) fn finish(self) -> NameTable {
        self.table
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::NameInterner;

    #[test]
    fn stores_each_distinct_name_once() {
        let mut interner = NameInterner::default();
        let first = interner.intern(OsStr::new("index.js"));
        let other = interner.intern(OsStr::new("package.json"));
        assert_eq!(interner.intern(OsStr::new("index.js")), first);
        let table = interner.finish();
        assert_eq!(table.get(first), "index.js");
        assert_eq!(table.get(other), "package.json");
    }
}
