#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Esc,
    Up,
    Down,

    W,
    R,
    G,
    B,
    P,
    Y,
    K,

    F,
    L,
    S,
    T,
    C,
    N,

    Z,

    LShift,

    Unknown(u32),
}

impl Key {
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => Self::Esc,
            103 => Self::Up,
            108 => Self::Down,

            17 => Self::W,
            19 => Self::R,
            34 => Self::G,
            48 => Self::B,
            25 => Self::P,
            21 => Self::Y,
            37 => Self::K,

            33 => Self::F,
            38 => Self::L,
            31 => Self::S,
            20 => Self::T,
            46 => Self::C,
            49 => Self::N,

            44 => Self::Z,

            42 => Self::LShift,

            _ => Self::Unknown(code),
        }
    }
}
