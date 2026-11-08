//! Minimal, auditable M5PM1 GPIO2 (L3B) power-control implementation.
//! Register operations were cross-checked against m5stack/M5PM1.
//! No reset, wake, charge, boost, or other persistent power settings are touched.

pub const ADDRESS: u8 = 0x6e;
const DEVICE_ID: u16 = 0x2050;
const GPIO_MODE: u8 = 0x10;
const GPIO_OUT: u8 = 0x11;
const GPIO_DRV: u8 = 0x13;
const GPIO_FUNC0: u8 = 0x16;
const L3B_BIT: u8 = 1 << 2;
const L3B_FUNC_MASK: u8 = 0b11 << 4;

pub trait RegisterBus {
    type Error;
    fn read(&mut self, register: u8) -> Result<u8, Self::Error>;
    fn write(&mut self, register: u8, value: u8) -> Result<(), Self::Error>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum PowerError<E> {
    Bus(E),
    WrongDevice { found: u16 },
    VerificationFailed { register: u8 },
}

fn change_bits<B: RegisterBus>(bus: &mut B, register: u8, mask: u8, bits: u8)
    -> Result<(), PowerError<B::Error>>
{
    let old = bus.read(register).map_err(PowerError::Bus)?;
    let expected = (old & !mask) | (bits & mask);
    if expected != old { bus.write(register, expected).map_err(PowerError::Bus)?; }
    let actual = bus.read(register).map_err(PowerError::Bus)?;
    if (actual & mask) != (expected & mask) {
        return Err(PowerError::VerificationFailed { register });
    }
    Ok(())
}

/// Enables only GPIO2-controlled L3B. Aborts before all writes if ID differs.
/// Prior GPIO2 configuration is not restored; don't call for power-down.
/// Exact electrical behavior requires validation on StickS3 hardware.
pub fn enable_l3b<B: RegisterBus>(bus: &mut B) -> Result<(), PowerError<B::Error>> {
    let lo = bus.read(0x00).map_err(PowerError::Bus)?;
    let hi = bus.read(0x01).map_err(PowerError::Bus)?;
    let found = u16::from_le_bytes([lo, hi]);
    if found != DEVICE_ID { return Err(PowerError::WrongDevice { found }); }
    // Preload enabled output level, select GPIO, push-pull, output direction.
    change_bits(bus, GPIO_OUT, L3B_BIT, L3B_BIT)?;
    change_bits(bus, GPIO_FUNC0, L3B_FUNC_MASK, 0)?;
    change_bits(bus, GPIO_DRV, L3B_BIT, 0)?;
    change_bits(bus, GPIO_MODE, L3B_BIT, L3B_BIT)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake { regs: [u8; 256], writes: Vec<(u8,u8)>, corrupt: Option<u8> }
    impl Fake {
        fn valid() -> Self { let mut regs=[0; 256]; regs[0]=0x50; regs[1]=0x20;
            Self { regs, writes: Vec::new(), corrupt: None } }
    }
    impl RegisterBus for Fake {
        type Error = ();
        fn read(&mut self, r: u8) -> Result<u8,()> {
            Ok(if self.corrupt==Some(r) { 0 } else { self.regs[r as usize] })
        }
        fn write(&mut self, r: u8, v: u8) -> Result<(),()> {
            self.regs[r as usize]=v; self.writes.push((r,v)); Ok(())
        }
    }
    #[test] fn identity_must_match_before_mutation() {
        let mut f=Fake::valid(); f.regs[1]=0;
        assert_eq!(enable_l3b(&mut f), Err(PowerError::WrongDevice { found: 0x50 }));
        assert!(f.writes.is_empty());
    }
    #[test] fn only_gpio2_fields_are_changed() {
        let mut f=Fake::valid(); f.regs[GPIO_MODE as usize]=0b10001;
        f.regs[GPIO_FUNC0 as usize]=0xff; f.regs[GPIO_DRV as usize]=0xff;
        enable_l3b(&mut f).unwrap();
        assert_eq!(f.regs[GPIO_MODE as usize], 0b10101);
        assert_eq!(f.regs[GPIO_OUT as usize] & L3B_BIT,L3B_BIT);
        assert_eq!(f.regs[GPIO_DRV as usize],0xfb);
        assert_eq!(f.regs[GPIO_FUNC0 as usize],0xcf);
        assert_eq!(f.writes.len(),4);
    }
    #[test] fn no_extra_writes_if_already_enabled() {
        let mut f=Fake::valid();enable_l3b(&mut f).unwrap();
        f.writes.clear();enable_l3b(&mut f).unwrap();assert!(f.writes.is_empty());
    }
    #[test] fn readback_failure_stops_sequence() {
        let mut f=Fake::valid();f.corrupt=Some(GPIO_OUT);
        assert_eq!(enable_l3b(&mut f),Err(PowerError::VerificationFailed{register:GPIO_OUT}));
        assert_eq!(f.writes.len(),1);
    }
}
