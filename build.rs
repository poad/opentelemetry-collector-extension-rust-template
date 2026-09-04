use vergen::EmitBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:warning=Running vergen build script");
    
    EmitBuilder::builder()
        .all_cargo()
        .all_rustc()
        .all_build()
        .emit()?;

    println!("cargo:warning=Vergen build script completed");
    Ok(())
}