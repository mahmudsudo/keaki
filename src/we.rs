//! # Extractable Witness Encryption Module
//!
//! This module contains the implementation of an Extractable Witness Encryption from an Extractable Witness KEM.'

#![allow(clippy::type_complexity)]
use crate::kem::{KEMError, KEM};
use ark_ec::pairing::Pairing;
use thiserror::Error;

/// Extractable Witness Encryption struct.
pub struct WE<E: Pairing> {
    kem: KEM<E>,
}

impl<E: Pairing> WE<E> {
    /// Create a new instance.
    pub fn new(kem: KEM<E>) -> Self {
        Self { kem }
    }

    /// Encrypts a message for a commitment and a set of points and values.
    /// Returns a vector of ciphertext tuples, in the order of the input points and values.
    pub fn encrypt(
        &self,
        com: E::G1,
        points: Vec<E::ScalarField>,
        values: Vec<E::ScalarField>,
        msg: &[u8],
    ) -> Result<Vec<(E::G2, Vec<u8>)>, WEError> {
        let mut cts = Vec::new();

        for i in 0..points.len() {
            let (key_ct, msg_ct) = self.encrypt_single(com, points[i], values[i], msg)?;
            cts.push((key_ct, msg_ct));
        }

        Ok(cts)
    }

    /// Encrypts a message using a commitment, point, and value.
    /// Returns two ciphertexts:
    /// - `key_ct`: used to generate the decryption key.
    /// - `msg_ct`: the encrypted message.
    pub fn encrypt_single(
        &self,
        com: E::G1,
        point: E::ScalarField,
        value: E::ScalarField,
        msg: &[u8],
    ) -> Result<(E::G2, Vec<u8>), WEError> {
        // Generate a key and the corresponding key ciphertext
        // (ct_1, k) <- Encap(x)
        let (key_ct, mut key_stream) = self.kem.encapsulate(com, point, value)?;

        // ct_2 <- Enc(k, m)
        let mut msg_ct = vec![0u8; msg.len()];
        key_stream.fill(&mut msg_ct);
        for i in 0..msg.len() {
            msg_ct[i] ^= msg[i];
        }

        // (ct_1, ct_2)
        Ok((key_ct, msg_ct))
    }

    /// Decrypts a ciphertext with a proof.
    /// Returns the decrypted message.
    pub fn decrypt_single(
        &self,
        proof: E::G1,
        key_ct: E::G2,
        msg_ct: &[u8],
    ) -> Result<Vec<u8>, WEError> {
        // k = Decap(w, ct_1)
        let mut key_stream = self.kem.decapsulate(proof, key_ct)?;

        // m = Dec(k, ct_2)
        let mut msg = vec![0u8; msg_ct.len()];
        key_stream.fill(&mut msg);
        for i in 0..msg_ct.len() {
            msg[i] ^= msg_ct[i];
        }

        Ok(msg)
    }
}

#[derive(Error, Debug)]
pub enum WEError {
    #[error("Key Encapsulation Error {0}")]
    KEMError(KEMError),
}

impl From<KEMError> for WEError {
    fn from(error: KEMError) -> Self {
        WEError::KEMError(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kzg::KZG;
    use crate::pol_op::evaluate_polynomial;
    use ark_bls12_381::{Bls12_381, Fr, G1Projective, G2Projective};
    use ark_std::test_rng;
    use ark_std::UniformRand;

    #[test]
    fn test_encrypt_single() {
        let rng = &mut test_rng();
        let g1_gen = G1Projective::rand(rng);
        let g2_gen = G2Projective::rand(rng);
        let secret = Fr::rand(rng);
        let max_degree = 10;
        let point: Fr = Fr::rand(rng);
        let kzg: KZG<Bls12_381> = KZG::setup(g1_gen, g2_gen, max_degree, secret);
        let kem: KEM<Bls12_381> = KEM::new(kzg);
        let we: WE<Bls12_381> = WE::new(kem);

        // p(x) = 7 x^4 + 9 x^3 - 5 x^2 - 25 x - 24
        let p = vec![
            Fr::from(-24),
            Fr::from(-25),
            Fr::from(-5),
            Fr::from(9),
            Fr::from(7),
        ];
        let val = evaluate_polynomial::<Bls12_381>(&p, &point);
        let commitment = we.kem.kzg().commit(&p).unwrap();

        let msg = b"helloworld";

        let (key_ct, msg_ct) = we.encrypt_single(commitment, point, val, msg).unwrap();

        let proof = we.kem.kzg().open(&p, &point).unwrap();

        let decrypted_msg = we.decrypt_single(proof, key_ct, &msg_ct).unwrap();

        assert_eq!(msg.to_vec(), decrypted_msg);
    }

    #[test]
    fn test_decrypt_single_invalid_proof() {
        let rng = &mut test_rng();
        let g1_gen = G1Projective::rand(rng);
        let g2_gen = G2Projective::rand(rng);
        let secret = Fr::rand(rng);
        let max_degree = 10;
        let point: Fr = Fr::rand(rng);
        let kzg: KZG<Bls12_381> = KZG::setup(g1_gen, g2_gen, max_degree, secret);
        let kem: KEM<Bls12_381> = KEM::new(kzg);
        let we: WE<Bls12_381> = WE::new(kem);

        // p(x) = 7 x^4 + 9 x^3 - 5 x^2 - 25 x - 24
        let p = vec![
            Fr::from(-24),
            Fr::from(-25),
            Fr::from(-5),
            Fr::from(9),
            Fr::from(7),
        ];
        let val = evaluate_polynomial::<Bls12_381>(&p, &point);
        let commitment = we.kem.kzg().commit(&p).unwrap();

        let msg = b"helloworld";
        let (key_ct, msg_ct) = we.encrypt_single(commitment, point, val, msg).unwrap();

        let wrong_point: Fr = Fr::rand(rng);
        let invalid_proof = we.kem.kzg().open(&p, &wrong_point).unwrap();

        let decrypted_msg = we.decrypt_single(invalid_proof, key_ct, &msg_ct).unwrap();

        assert_ne!(msg.to_vec(), decrypted_msg);
    }

    #[test]
    fn test_encrypt_decrypt_single() {
        let rng = &mut test_rng();
        let g1_gen = G1Projective::rand(rng);
        let g2_gen = G2Projective::rand(rng);
        let secret = Fr::rand(rng);
        let max_degree = 10;
        let kzg: KZG<Bls12_381> = KZG::setup(g1_gen, g2_gen, max_degree, secret);
        let kem: KEM<Bls12_381> = KEM::new(kzg);
        let we: WE<Bls12_381> = WE::new(kem);

        let p = vec![
            Fr::from(-24),
            Fr::from(-25),
            Fr::from(-5),
            Fr::from(9),
            Fr::from(7),
        ];
        let points = vec![Fr::rand(rng), Fr::rand(rng), Fr::rand(rng)];
        let values: Vec<Fr> = points
            .iter()
            .map(|&point| evaluate_polynomial::<Bls12_381>(&p, &point))
            .collect();
        let commitment = we.kem.kzg().commit(&p).unwrap();

        let msg = b"helloworld";

        let cts = we
            .encrypt(commitment, points.clone(), values.clone(), msg)
            .unwrap();

        let target_index = 1;
        let proof = we.kem.kzg().open(&p, &points[target_index]).unwrap();

        let (correct_key_ct, correct_msg_ct) = &cts[target_index];
        let decrypted_msg = we
            .decrypt_single(proof, *correct_key_ct, correct_msg_ct)
            .unwrap();
        assert_eq!(msg.to_vec(), decrypted_msg);

        // Attempt to decrypt a different message with the same proof
        let (wrong_key_ct, wrong_msg_ct) = &cts[0];
        let wrong_decrypted_msg = we
            .decrypt_single(proof, *wrong_key_ct, wrong_msg_ct)
            .unwrap();

        assert_ne!(msg.to_vec(), wrong_decrypted_msg);
    }
}