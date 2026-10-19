use windows::{
    core::Result,
    Globalization::Language,
    Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    Storage::Streams::DataWriter,
};

pub fn languages() -> Result<Vec<(String, String)>> {
    OcrEngine::AvailableRecognizerLanguages()?
        .into_iter()
        .map(|language| {
            Ok((
                language.LanguageTag()?.to_string(),
                language.DisplayName()?.to_string(),
            ))
        })
        .collect()
}

pub fn recognize(pixels: &[u8], width: i32, height: i32, language: Option<&str>) -> Result<String> {
    let engine = match language {
        Some(tag) => OcrEngine::TryCreateFromLanguage(&Language::CreateLanguage(&tag.into())?)?,
        None => OcrEngine::TryCreateFromUserProfileLanguages()?,
    };
    let max = OcrEngine::MaxImageDimension()? as i32;
    if width > max || height > max {
        return Err(windows::core::Error::new(
            windows::core::HRESULT(0x80070057u32 as i32),
            format!("Selection exceeds the OCR limit of {max} pixels. Select a smaller area."),
        ));
    }
    let writer = DataWriter::new()?;
    writer.WriteBytes(pixels)?;
    let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &writer.DetachBuffer()?,
        BitmapPixelFormat::Bgra8,
        width,
        height,
        BitmapAlphaMode::Ignore,
    )?;
    let result = engine.RecognizeAsync(&bitmap)?.get()?;
    let mut lines = Vec::new();
    for line in result.Lines()? {
        lines.push(line.Text()?.to_string());
    }
    Ok(lines.join("\r\n").trim().to_owned())
}
