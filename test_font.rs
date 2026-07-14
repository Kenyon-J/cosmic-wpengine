use cosmic_text::FontSystem;
fn main() {
    let font_system = FontSystem::new();
    println!("Fonts found: {}", font_system.db().len());
}
