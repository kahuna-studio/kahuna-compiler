pub mod envelopes;

#[cfg(test)]
mod tests {
    use kahuna_core::Audio;
    use kahuna_module::kahuna_module;

    // Test-only modules are skipped, so this is not part of the manifest.
    #[kahuna_module]
    pub struct TestOnly {
        #[output]
        out: Audio,
    }
}
