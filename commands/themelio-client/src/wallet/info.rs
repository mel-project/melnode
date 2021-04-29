use colored::Colorize;
use std::io::Write;
use tabwriter::TabWriter;

use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct CreatedWalletInfo {
    pub name: String,
    pub address: String,
    pub secret: String,
}

trait Printable {
    fn print(&self, w: &mut dyn std::io::Write);
}

impl Printable for CreatedWalletInfo {
    fn print(&self, w: &mut dyn Write) {
        let mut tw = TabWriter::new(vec![]);
        let name = self.name.clone();
        let addr = self.address.clone();
        let secret = self.secret.clone();

        writeln!(tw, ">> New data:\t{}", name.bold()).unwrap();
        writeln!(tw, ">> Address:\t{}", addr.yellow()).unwrap();
        writeln!(tw, ">> Secret:\t{}", secret.dimmed()).unwrap();

        let info = String::from_utf8(tw.into_inner().unwrap()).unwrap();
        write!(w, "{}", &info);
    }
}
