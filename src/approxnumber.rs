use std::hash::Hash;

pub struct ApproxNumber(pub f64,pub i32);

impl ApproxNumber {
    fn parts(&self) -> (i32,i64) {
        let log = (self.0).abs().log10();
        if log.is_infinite() { return (0,0); }
        let log = log.floor() as i32;
        let mul = (10_f64).powi(self.1-log-1);
        let x = (self.0*mul).round() as i64;
        (log,x)
    }
}

impl Hash for ApproxNumber {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let (log,x) = self.parts();
        log.hash(state);
        x.hash(state);
    }
}

impl PartialEq for ApproxNumber {
    fn eq(&self, other: &Self) -> bool {
        self.parts() == other.parts()
    }
}

impl Eq for ApproxNumber {}