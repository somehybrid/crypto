// A Rust implementation of XChaCha-Poly1305
// This implementation defaults to 20 rounds
use crate::backends;
use crate::utils::*;

use crate::poly1305::Poly1305;
use pyo3::exceptions::PyAssertionError;
use pyo3::prelude::*;
use std::borrow::Cow;

#[pyclass]
pub struct ChaCha {
    key: Vec<u8>,
    rounds: usize,
}

impl ChaCha {
    fn keystream(&self, nonce: &[u8], counter: u32) -> [u8; 128] {
        let mut state: [[u32; 4]; 4] = [
            [0x61707865, 0x3320646e, 0x79622d32, 0x6b206574],
            [
                from_le_bytes(&self.key[0..4]),
                from_le_bytes(&self.key[4..8]),
                from_le_bytes(&self.key[8..12]),
                from_le_bytes(&self.key[12..16]),
            ],
            [
                from_le_bytes(&self.key[16..20]),
                from_le_bytes(&self.key[20..24]),
                from_le_bytes(&self.key[24..28]),
                from_le_bytes(&self.key[28..]),
            ],
            [
                counter,
                from_le_bytes(&nonce[0..4]),
                from_le_bytes(&nonce[4..8]),
                from_le_bytes(&nonce[8..12]),
            ],
        ];

        backends::rounds(state.clone(), self.rounds, false)
    }
}

#[pymethods]
impl ChaCha {
    #[new]
    pub fn new(key: Vec<u8>, r: Option<usize>) -> PyResult<ChaCha> {
        let rounds;

        if r.is_some() {
            rounds = r.unwrap();
        } else {
            rounds = 20;
        }

        if key.len() != 32 {
            return Err(PyAssertionError::new_err("Key must be 32 bytes in length."));
        }

        if rounds < 1 {
            return Err(PyAssertionError::new_err("Rounds must be at least 1"));
        }

        Ok(ChaCha { key, rounds })
    }

    pub fn encrypt(&self, plaintext: &[u8], nonce: &[u8], counter: u32) -> PyResult<Vec<u8>> {
        if nonce.len() != 12 {
            return Err(PyAssertionError::new_err(
                "Nonce must be 12 bytes in length.",
            ));
        }

        let mut ciphertext: Vec<u8> = Vec::new();

        for (index, block) in plaintext.chunks(128).enumerate() {
            let keystream = self.keystream(nonce, counter + index as u32);

            for (key, chunk) in block.iter().zip(keystream) {
                ciphertext.push(chunk ^ key);
            }
        }

        Ok(ciphertext)
    }
}

// ChaCha-Poly1305 implementation
#[pyclass]
pub struct ChaChaPoly1305 {
    key: Vec<u8>,
    rounds: usize,
}

#[pymethods]
impl ChaChaPoly1305 {
    #[new]
    pub fn new(key: Vec<u8>, r: Option<usize>) -> PyResult<ChaChaPoly1305> {
        let rounds;

        if r.is_some() {
            rounds = r.unwrap();
        } else {
            rounds = 20;
        }

        if key.len() != 32 {
            return Err(PyAssertionError::new_err("Key must be 32 bytes in length."));
        }

        if rounds < 1 {
            return Err(PyAssertionError::new_err("Rounds must be at least 1"));
        }

        Ok(ChaChaPoly1305 { key, rounds })
    }

    pub fn encrypt(
        &self,
        plaintext: &[u8],
        nonce: &[u8],
        aad: &[u8],
        counter: u32,
    ) -> PyResult<Vec<u8>> {
        let chacha = ChaCha::new(self.key.clone(), Some(self.rounds))?;

        let otk = &chacha.keystream(nonce, 0);
        let poly1305_key = otk[..32].to_vec();

        let mut poly1305 = Poly1305::new(poly1305_key);
        let ciphertext = chacha.encrypt(plaintext, nonce, counter)?;

        poly1305.update(aad);
        poly1305.update(&ciphertext);
        let aad_len = aad.len() as u64;
        let ciphertext_len = ciphertext.len() as u64;
        let mut lens = Vec::new();

        lens.extend_from_slice(&aad_len.to_le_bytes());
        lens.extend_from_slice(&ciphertext_len.to_le_bytes());

        poly1305.update(&lens);

        Ok([ciphertext, poly1305.tag()].concat())
    }

    pub fn decrypt(
        &self,
        text: &[u8],
        nonce: &[u8],
        aad: &[u8],
        counter: u32,
    ) -> PyResult<Vec<u8>> {
        if text.len() < 17 {
            return Err(PyAssertionError::new_err("Invalid ciphertext"));
        }

        let ciphertext = &text[..text.len() - 16];
        let tag = &text[text.len() - 16..];
        let chacha = ChaCha::new(self.key.clone(), Some(self.rounds))?;

        let otk = &chacha.keystream(nonce, 0);
        let poly1305_key = otk[..32].to_vec();

        let mut poly1305 = Poly1305::new(poly1305_key);
        let plaintext = chacha.encrypt(ciphertext, nonce, counter)?;

        poly1305.update(&ciphertext);
        poly1305.update(&aad);

        let aad_len = aad.len() as u64;
        let ciphertext_len = ciphertext.len() as u64;
        let mut lens = Vec::new();

        lens.extend_from_slice(&aad_len.to_le_bytes());
        lens.extend_from_slice(&ciphertext_len.to_le_bytes());

        poly1305.update(&lens);

        if !poly1305.verify(tag) {
            return Err(PyAssertionError::new_err("Invalid MAC"));
        }

        Ok(plaintext.to_vec())
    }
}

pub fn hchacha(key: &[u8], nonce: &[u8], rounds: usize) -> Vec<u8> {
    let mut state: [[u32; 4]; 4] = [
        [0x61707865, 0x3320646e, 0x79622d32, 0x6b206574],
        [
            from_le_bytes(&key[0..4]),
            from_le_bytes(&key[4..8]),
            from_le_bytes(&key[8..12]),
            from_le_bytes(&key[12..16]),
        ],
        [
            from_le_bytes(&key[16..20]),
            from_le_bytes(&key[20..24]),
            from_le_bytes(&key[24..28]),
            from_le_bytes(&key[28..]),
        ],
        [
            from_le_bytes(&nonce[0..4]),
            from_le_bytes(&nonce[4..8]),
            from_le_bytes(&nonce[8..12]),
            from_le_bytes(&nonce[12..]),
        ],
    ];

    let data = backends::rounds(state, rounds, true);

    [&data[0..16], &data[48..64]].concat().to_vec()
}

#[pyclass]
pub struct XChaChaPoly1305 {
    key: Vec<u8>,
    rounds: usize,
}

#[pymethods]
impl XChaChaPoly1305 {
    #[new]
    pub fn new(key: Vec<u8>, r: Option<usize>) -> PyResult<XChaChaPoly1305> {
        let rounds;

        if r.is_some() {
            rounds = r.unwrap();
        } else {
            rounds = 20;
        }

        if key.len() != 32 {
            return Err(PyAssertionError::new_err("Key must be 32 bytes in length."));
        }

        if rounds < 1 {
            return Err(PyAssertionError::new_err("Rounds must be at least 1"));
        }

        Ok(XChaChaPoly1305 { key, rounds })
    }

    fn key(&self, nonce: &[u8]) -> (Vec<u8>, [u8; 12]) {
        let mut chacha_nonce = [0u8; 12];
        chacha_nonce[4..].copy_from_slice(&nonce[16..24]);

        let subkey = hchacha(&self.key, &nonce[..16], self.rounds);

        (subkey, chacha_nonce)
    }

    pub fn encrypt(
        &self,
        plaintext: &[u8],
        nonce: &[u8],
        aad: &[u8],
        counter: u32,
    ) -> PyResult<Vec<u8>> {
        let (subkey, chacha_nonce) = self.key(nonce);

        let chacha = ChaChaPoly1305::new(subkey, Some(self.rounds))?;

        chacha
            .encrypt(plaintext, &chacha_nonce, aad, counter)
            .into()
    }

    pub fn decrypt(
        &self,
        ciphertext: &[u8],
        nonce: &[u8],
        aad: &[u8],
        counter: u32,
    ) -> PyResult<Vec<u8>> {
        let (subkey, chacha_nonce) = self.key(nonce);

        let chacha = ChaChaPoly1305::new(subkey, Some(self.rounds))?;

        chacha.decrypt(ciphertext, &chacha_nonce, aad, counter)
    }
}

#[pyfunction]
pub fn encrypt(
    key: Vec<u8>,
    plaintext: Vec<u8>,
    iv: Option<Vec<u8>>,
    data: Option<Vec<u8>>,
    counter: Option<u32>,
    rounds: Option<usize>,
) -> PyResult<Cow<'static, [u8]>> {
    let cipher = XChaChaPoly1305::new(key.clone(), rounds.clone())?;

    let nonce = iv.unwrap_or(vec![0u8; 24]);
    let ctr = counter.unwrap_or(1);
    let aad = data.unwrap_or_default();

    let data = cipher.encrypt(&plaintext, &nonce, &aad, ctr)?;

    Ok(data.into())
}

#[pyfunction]
pub fn decrypt(
    key: Vec<u8>,
    plaintext: Vec<u8>,
    iv: Option<Vec<u8>>,
    data: Option<Vec<u8>>,
    counter: Option<u32>,
    rounds: Option<usize>,
) -> PyResult<Cow<'static, [u8]>> {
    let cipher = XChaChaPoly1305::new(key, rounds)?;

    let nonce = iv.unwrap_or(vec![0u8; 24]);
    let ctr = counter.unwrap_or(1);
    let aad = data.unwrap_or_default();

    let data = cipher.decrypt(&plaintext, &nonce, &aad, ctr)?;

    Ok(data.into())
}
