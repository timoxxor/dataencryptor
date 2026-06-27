/*
use ring::{error};
use ring::digest::{SHA256, digest};

mod crypto;

fn main() -> Result<(), error::Unspecified> {


    // let bytes = crypto::populate_nonce()?;

    // let mut array16 = [0u8; 16];
    // array16[4..].copy_from_slice(&bytes);
    // let num = u128::from_be_bytes(array16);
    // println!("{}", num);

    let pwd = digest(&SHA256, b"password");
    for i in [0, 1, 2] {
        let k = crypto::hkdf_derive(None, format!("file_index{}", i).as_bytes(), &pwd.as_ref().to_vec())?;
        println!("k{}:{:?}", i, k);
    }

    // let alg = crypto::Aes::new(pwd.as_ref().to_vec(), None);
    // let e = alg.encrypt(b"pisun")?;

    // println!("encrypted data = {:?}", e);
    // println!("decrypted_data = {:?}", String::from_utf8(alg.decrypt(e.as_slice())?).unwrap());

    Ok(())
}
*/

use eframe::egui;
mod crypto;
mod deflate;
mod gui;
mod particles;
mod gif_player;
mod ui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("EVFS - Encrypted Virtual File System")
            .with_inner_size([900.0, 650.0])
            .with_min_inner_size([700.0, 500.0])
            .with_decorations(false)
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "EVFS App",
        options,
        Box::new(|_cc| Ok(Box::new(gui::FileBrowserApp::default()))),
    )
}
