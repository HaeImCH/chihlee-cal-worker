use std::path::Path;

use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, dictionary};

pub fn create_test_pdf(path: &Path, pages: &[Vec<&str>]) -> Result<(), Box<dyn std::error::Error>> {
    let mut doc = Document::with_version("1.5");

    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
        },
    });

    let mut page_ids = Vec::new();

    for lines in pages {
        let mut operations = vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 12.into()]),
            Operation::new("TL", vec![16.into()]),
            Operation::new("Td", vec![50.into(), 780.into()]),
        ];

        for (index, line) in lines.iter().enumerate() {
            operations.push(Operation::new("Tj", vec![Object::string_literal(*line)]));
            if index + 1 < lines.len() {
                operations.push(Operation::new("T*", vec![]));
            }
        }
        operations.push(Operation::new("ET", vec![]));

        let content = Content { operations };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode()?));

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        page_ids.push(page_id);
    }

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids.iter().map(|id| (*id).into()).collect::<Vec<_>>(),
            "Count" => i64::try_from(page_ids.len())?,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.compress();

    doc.save(path)?;
    Ok(())
}
