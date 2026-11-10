use logisim_plugin_api::Signal;
pub(super) struct State {
    pub clock: u8,
    pub revision: u64,
    pub back: Vec<u8>,
    pub front: Vec<u8>,
}
pub(super) fn known(s: Signal) -> Option<u32> {
    (s.unknown == 0 && s.error == 0).then_some(s.value)
}
impl State {
    pub fn new(side: usize) -> Self {
        Self {
            clock: 2,
            revision: 0,
            back: vec![0; side * side],
            front: vec![0; side * side],
        }
    }
    pub fn decode(bytes: &[u8], side: usize) -> Result<Self, String> {
        if bytes.is_empty() {
            return Ok(Self::new(side));
        }
        if bytes.len() != 10 + 2 * side * side || bytes[0] != 1 || bytes[1] > 2 {
            return Err("Invalid framebuffer state.".into());
        }
        Ok(Self {
            clock: bytes[1],
            revision: u64::from_le_bytes(bytes[2..10].try_into().expect("checked length")),
            back: bytes[10..10 + side * side].to_vec(),
            front: bytes[10 + side * side..].to_vec(),
        })
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![1, self.clock];
        out.extend_from_slice(&self.revision.to_le_bytes());
        out.extend_from_slice(&self.back);
        out.extend_from_slice(&self.front);
        out
    }
    pub fn sample(&mut self, inputs: &[Signal], side: usize) -> Result<(), String> {
        let clock = known(inputs[0]).unwrap_or(2) as u8;
        let edge = self.clock == 0 && clock == 1;
        self.clock = clock;
        if !edge {
            return Ok(());
        }
        if known(inputs[5]) == Some(1) {
            if let Some(color) = known(inputs[3]) {
                self.back.fill(color as u8);
            }
        } else if known(inputs[4]) == Some(1)
            && let (Some(x), Some(y), Some(color)) =
                (known(inputs[1]), known(inputs[2]), known(inputs[3]))
            && (x as usize) < side
            && (y as usize) < side
        {
            self.back[y as usize * side + x as usize] = color as u8;
        }
        if known(inputs[6]) == Some(1) && self.front != self.back {
            self.revision = self
                .revision
                .checked_add(1)
                .ok_or("Framebuffer revision exhausted; reset required.")?;
            self.front.clone_from(&self.back);
        }
        Ok(())
    }
}
