use crate::{common, ARGS};

const FILENAME: &str = "rust-toolchain";
const CONTENTS: &str = include_str!("../resources/rust-toolchain.in");

pub fn create() {
    common::write_file(ARGS.root_path().join(FILENAME), CONTENTS);
}

#[cfg(test)]
mod tests {
    use reqwest::blocking;

    use super::CONTENTS;

    const DIMENSION_NODE_TOOLCHAIN_URL: &str =
        "https://raw.githubusercontent.com/dimension-labs/dimension-node/main/smart_contracts/rust-toolchain";

    #[test]
    fn check_toolchain_version() {
        let expected_toolchain_value = blocking::get(DIMENSION_NODE_TOOLCHAIN_URL)
            .expect("should get rust-toolchain from GitHub")
            .text()
            .expect("should parse rust-toolchain");

        // If this fails, ensure there's not a mismatch between ../resources/rust-toolchain.in and
        // https://github.com/dimension-labs/dimension-node/blob/main/smart_contracts/rust-toolchain.
        assert_eq!(&*expected_toolchain_value, CONTENTS);
    }
}
