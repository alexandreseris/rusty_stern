pub use rusty_stern_macros::Update;

pub trait Update {
    /// update all fields of `self` using `other` object
    fn update_from(&mut self, other_setting: Self);
}
