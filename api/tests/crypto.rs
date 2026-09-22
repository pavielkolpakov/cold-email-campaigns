use api::crypto::Cipher;

fn cipher() -> Cipher {
    Cipher::from_base64_key("bTfLDZ0kFqfhy5qTmdsmMxTjy0/6sdtIBOFkDFCKVGE=").unwrap()
}

#[test]
fn encrypted_text_decrypts_back_to_the_original() {
    let cipher = cipher();
    let token = "1//0gRefreshTokenFromGoogle";

    let sealed = cipher.encrypt(token).unwrap();
    assert_eq!(cipher.decrypt(&sealed).unwrap(), token);
}

#[test]
fn the_same_secret_encrypts_differently_every_time() {
    let cipher = cipher();
    let token = "1//0gRefreshTokenFromGoogle";

    let first = cipher.encrypt(token).unwrap();
    let second = cipher.encrypt(token).unwrap();

    assert_ne!(first, second, "a reused nonce would break AES-GCM");
    assert!(!first.contains("RefreshToken"));
    assert_eq!(
        cipher.decrypt(&first).unwrap(),
        cipher.decrypt(&second).unwrap()
    );
}

#[test]
fn a_different_key_cannot_decrypt() {
    let sealed = cipher().encrypt("1//0gRefreshTokenFromGoogle").unwrap();

    let other = Cipher::from_base64_key("FCgYZ8hQ9V0m0zqk0Z7Yx7q3H9kQx1nT5cW2pL8vA4o=").unwrap();

    assert!(other.decrypt(&sealed).is_err());
}

#[test]
fn a_key_of_the_wrong_length_is_rejected() {
    assert!(Cipher::from_base64_key("c2hvcnQ=").is_err());
}
