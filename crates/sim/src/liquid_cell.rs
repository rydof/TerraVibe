#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LiquidKind {
    Water,
    Lava,
    Honey,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Liquid {
    pub kind: LiquidKind,
    pub amount: u8, // 1..=255; 0 means none
}

impl Liquid {
    pub const fn new(kind: LiquidKind, amount: u8) -> Self {
        Self { kind, amount }
    }

    pub const fn water(amount: u8) -> Self {
        Self {
            kind: LiquidKind::Water,
            amount,
        }
    }

    pub const fn lava(amount: u8) -> Self {
        Self {
            kind: LiquidKind::Lava,
            amount,
        }
    }

    pub const fn honey(amount: u8) -> Self {
        Self {
            kind: LiquidKind::Honey,
            amount,
        }
    }

    pub const fn pack(l: Option<Liquid>) -> u16 {
        match l {
            Some(Liquid { amount, .. }) if amount == 0 => 0,
            Some(Liquid { kind, amount }) => {
                let kind_bits = match kind {
                    LiquidKind::Water => 0u16,
                    LiquidKind::Lava => 1u16,
                    LiquidKind::Honey => 2u16,
                };
                (kind_bits << 8) | (amount as u16)
            }
            None => 0,
        }
    }

    pub const fn unpack(raw: u16) -> Option<Liquid> {
        let amount = (raw & 0xFF) as u8;
        if amount == 0 {
            return None;
        }
        let kind = match (raw >> 8) & 3 {
            1 => LiquidKind::Lava,
            2 => LiquidKind::Honey,
            _ => LiquidKind::Water,
        };
        Some(Liquid { kind, amount })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack_roundtrip() {
        assert_eq!(Liquid::pack(None), 0);
        assert_eq!(Liquid::pack(Some(Liquid::water(0))), 0);
        assert_eq!(Liquid::pack(Some(Liquid::lava(0))), 0);
        assert_eq!(Liquid::unpack(0), None);

        let w = Liquid::water(200);
        assert_eq!(Liquid::unpack(Liquid::pack(Some(w))), Some(w));

        let l = Liquid::lava(255);
        assert_eq!(Liquid::unpack(Liquid::pack(Some(l))), Some(l));
    }
}
