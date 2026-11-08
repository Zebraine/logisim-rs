//! Reusable dense membership with sparse iteration and clearing.
#[derive(Default)]
pub(crate) struct WorkSet {
    marked: Vec<bool>,
    pub items: Vec<usize>,
}
impl WorkSet {
    pub fn new(len: usize) -> Self {
        Self {
            marked: vec![false; len],
            items: vec![],
        }
    }
    pub fn insert(&mut self, index: usize) {
        if !std::mem::replace(&mut self.marked[index], true) {
            self.items.push(index);
        }
    }
    pub fn contains(&self, index: usize) -> bool {
        self.marked[index]
    }
    pub fn clear(&mut self) {
        for index in self.items.drain(..) {
            self.marked[index] = false;
        }
    }
}
