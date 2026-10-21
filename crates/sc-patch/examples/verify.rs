//! `cargo run -p sc-patch --example verify -- <set.patchset.toml> <exe>`:
//! checks a patchset against a file, as the launcher does before starting.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [set, exe] = args.as_slice() else {
        eprintln!("usage: verify <set.patchset.toml> <exe>");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(set).expect("read patchset");
    let set = sc_patch::Patchset::parse(&text, set).unwrap_or_else(|e| panic!("{e}"));
    let r = sc_patch::verify_file(&set, std::fs::read(exe).expect("read exe"), None);
    println!("{} patches; ok {:?}\nskipped {:?}\nerrors {:?}\nwarnings {:?}", set.patches.len(), r.ok, r.skipped, r.errors, r.warnings);
}
