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

fn recognize_legacy(
    pixels: &[u8],
    width: i32,
    height: i32,
    language: Option<&str>,
) -> Result<String> {
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

pub struct Request {
    pub pixels: Vec<u8>,
    pub width: i32,
    pub height: i32,
    pub language: Option<String>,
    pub prefer_oneocr: bool,
}

pub struct Recognition {
    pub text: String,
    pub engine: &'static str,
    pub fallback_reason: Option<String>,
}

pub enum Event {
    Ready(String),
    Completed(std::result::Result<Recognition, String>),
}

// This object and its native handles stay on one dedicated MTA worker thread.
pub struct Backend {
    oneocr: std::result::Result<crate::oneocr::Engine, String>,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            oneocr: crate::oneocr::Engine::discover(),
        }
    }

    pub fn status(&self) -> String {
        match &self.oneocr {
            Ok(engine) => format!("OneOCR ready. Automatic language recognition.\nLoaded from: {}", engine.directory.display()),
            Err(error) => format!("OneOCR unavailable: {error}\nWindows.Media.Ocr fallback will be used; its language packs are required."),
        }
    }

    pub fn recognize(&self, request: Request) -> std::result::Result<Recognition, String> {
        route(
            request.prefer_oneocr,
            || match &self.oneocr {
                Ok(engine) => engine.recognize(&request.pixels, request.width, request.height),
                Err(error) => Err(error.clone()),
            },
            || {
                recognize_legacy(
                    &request.pixels,
                    request.width,
                    request.height,
                    request.language.as_deref(),
                )
                .map_err(|error| {
                    format!("Windows OCR failed: {error}. Check Windows OCR language packs.")
                })
            },
        )
    }
}

fn route(
    prefer_oneocr: bool,
    oneocr: impl FnOnce() -> std::result::Result<String, String>,
    legacy: impl FnOnce() -> std::result::Result<String, String>,
) -> std::result::Result<Recognition, String> {
    let fallback_reason = if prefer_oneocr {
        match oneocr() {
            Ok(text) => {
                return Ok(Recognition {
                    text,
                    engine: "OneOCR",
                    fallback_reason: None,
                })
            }
            Err(reason) => Some(reason),
        }
    } else {
        None
    };
    match legacy() {
        Ok(text) => Ok(Recognition {
            text,
            engine: "Windows.Media.Ocr",
            fallback_reason,
        }),
        Err(error) => Err(match fallback_reason {
            Some(reason) => format!("OneOCR: {reason}\n\n{error}"),
            None => error,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oneocr_success_including_empty_result_does_not_run_fallback() {
        for text in ["中文 English", ""] {
            let result = route(
                true,
                || Ok(text.into()),
                || panic!("unexpected legacy call"),
            )
            .unwrap();
            assert_eq!(result.text, text);
            assert_eq!(result.engine, "OneOCR");
        }
    }
    #[test]
    fn oneocr_failure_keeps_reason_and_returns_legacy_text() {
        let result = route(
            true,
            || Err("DLL missing".into()),
            || Ok("fallback text".into()),
        )
        .unwrap();
        assert_eq!(result.text, "fallback text");
        assert_eq!(result.engine, "Windows.Media.Ocr");
        assert_eq!(result.fallback_reason.as_deref(), Some("DLL missing"));
    }
    #[test]
    fn manual_legacy_selection_bypasses_oneocr() {
        let result = route(
            false,
            || panic!("unexpected OneOCR call"),
            || Ok("legacy".into()),
        )
        .unwrap();
        assert!(result.fallback_reason.is_none());
    }
    #[test]
    fn both_failures_are_reported() {
        let error = route(
            true,
            || Err("model incompatible".into()),
            || Err("no language".into()),
        )
        .err()
        .unwrap();
        assert!(error.contains("model incompatible"));
        assert!(error.contains("no language"));
    }
}
