#[test]
fn x11_input_method_text_outside_utf8_is_decoded() -> Result<(), xim_ctext::DecodeError> {
    assert_eq!(xim_ctext::compound_text_to_utf8(&[0xDF])?, "ß");
    assert_eq!(xim_ctext::compound_text_to_utf8(b"\x1b-b\xa4")?, "€");
    assert_eq!(
        xim_ctext::compound_text_to_utf8(b"\x1b%G\xc3\xa9\x1b%@")?,
        "é"
    );
    Ok(())
}
