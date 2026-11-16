fn main() {
    // Las vistas GPUI de debug construyen árboles grandes en el hilo principal.
    // La reserva predeterminada de MSVC (1 MiB) puede desbordarse al renderizar.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg-bin=desktop=/STACK:16777216");
    }
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=assets/app.rc");
    #[cfg(windows)]
    embed_resource::compile("assets/app.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("No se pudo incorporar el icono de la aplicación");
}
