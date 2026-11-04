#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DatId(pub [u8; 4]);

impl DatId {
    pub fn from_name(name: &[u8; 4]) -> Self {
        DatId(*name)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        let mut id = [0u8; 4];
        for (i, b) in s.bytes().take(4).enumerate() {
            id[i] = b;
        }
        DatId(id)
    }

    pub fn as_str(&self) -> String {
        self.0.iter().map(|&b| b as char).collect()
    }

    pub fn final_char(&self) -> char {
        self.0[3] as char
    }

    pub fn final_digit(&self) -> Option<u32> {
        (self.0[3] as char).to_digit(10)
    }

    pub fn is_parameterized(&self) -> bool {
        self.0[3] == b'?'
    }

    pub fn parameterized_match(&self, other: &DatId) -> bool {
        if other.is_parameterized() {
            self.0[0..3] == other.0[0..3]
        } else {
            self.0 == other.0
        }
    }

    pub fn starts_with(&self, prefix: &str) -> bool {
        let p = prefix.as_bytes();
        p.len() <= 4 && self.0[..p.len()] == *p
    }
}
