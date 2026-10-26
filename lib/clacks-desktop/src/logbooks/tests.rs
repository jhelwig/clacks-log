use crate::logbooks::TagEditor;
use clacks_model::{
    ids::TagId,
    tag::{AutomaticUploadPolicy, LogbookTag},
};

#[test]
fn tag_editor_preserves_text_and_initializes_neutral_policies() {
    let id = TagId::new();
    let mut editor = TagEditor::new(id);
    assert_eq!(editor.automatic_upload.lotw, AutomaticUploadPolicy::Neither);
    assert_eq!(editor.automatic_upload.qrz, AutomaticUploadPolicy::Neither);
    assert!(editor.build().is_err());
    editor.name = "  Main  ".into();
    let tag = editor.build().unwrap();
    assert_eq!(tag, LogbookTag::new(id, "  Main  ".into()));
    assert_eq!(TagEditor::from_tag(&tag).build().unwrap(), tag);
    editor.name = " \t\n".into();
    assert_eq!(
        editor.build().unwrap_err().first().unwrap().field,
        "Logbook name"
    );
}
