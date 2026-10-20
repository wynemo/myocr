// Runtime-loaded OneOCR ABI, based on wangfu91/oneocr-rs (MIT) and
// b1tg/win11-oneocr. See THIRD_PARTY_NOTICES.md. No Microsoft DLL/model is bundled.
use std::{
    ffi::{c_char, c_void, CStr, CString},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};
use windows::{core::HSTRING, Management::Deployment::PackageManager};
use windows_sys::Win32::{
    Foundation::{FreeLibrary, HMODULE},
    System::LibraryLoader::{
        GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
        LOAD_LIBRARY_SEARCH_SYSTEM32,
    },
};

type Handle = *mut c_void;
type Create = unsafe extern "system" fn(*mut Handle) -> i32;
type Release = unsafe extern "system" fn(Handle);
type SetDelay = unsafe extern "system" fn(Handle, u8) -> i32;
type CreatePipeline =
    unsafe extern "system" fn(*const c_char, *const c_char, Handle, *mut Handle) -> i32;
type SetMaxLines = unsafe extern "system" fn(Handle, i32) -> i32;
type Run = unsafe extern "system" fn(Handle, *const RawImage, Handle, *mut Handle) -> i32;
type LineCount = unsafe extern "system" fn(Handle, *mut i64) -> i32;
type GetLine = unsafe extern "system" fn(Handle, i64, *mut Handle) -> i32;
type LineText = unsafe extern "system" fn(Handle, *mut *const c_char) -> i32;

#[repr(C)]
struct RawImage {
    format: i32,
    width: i32,
    height: i32,
    reserved: i32,
    stride: i64,
    data: i64,
}

struct Library(HMODULE);
impl Library {
    unsafe fn open(path: &Path) -> Result<Self, String> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // Resolve dependencies in the selected DLL's directory and System32 only.
        let module = LoadLibraryExW(
            wide.as_ptr(),
            null_mut(),
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
        );
        if module.is_null() {
            Err(format!(
                "Cannot load {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ))
        } else {
            Ok(Self(module))
        }
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.0);
        }
    }
}

struct Api {
    create_init: Create,
    set_delay: SetDelay,
    create_pipeline: CreatePipeline,
    create_process: Create,
    set_max_lines: SetMaxLines,
    run: Run,
    line_count: LineCount,
    get_line: GetLine,
    line_text: LineText,
    release_result: Release,
    release_process: Release,
    release_pipeline: Release,
    release_init: Release,
    // Drop the OCR DLL before its ONNX dependency, after all native handles.
    _library: Option<Library>,
    _runtime: Option<Library>,
}

impl Api {
    unsafe fn load(directory: &Path) -> Result<Self, String> {
        let runtime = Library::open(&directory.join("onnxruntime.dll"))?;
        let library = Library::open(&directory.join("oneocr.dll"))?;
        macro_rules! symbol {
            ($name:literal, $ty:ty) => {{
                let address = GetProcAddress(library.0, concat!($name, "\0").as_ptr())
                    .ok_or_else(|| format!("OneOCR export missing: {}", $name))?;
                std::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(address)
            }};
        }
        Ok(Self {
            create_init: symbol!("CreateOcrInitOptions", Create),
            set_delay: symbol!("OcrInitOptionsSetUseModelDelayLoad", SetDelay),
            create_pipeline: symbol!("CreateOcrPipeline", CreatePipeline),
            create_process: symbol!("CreateOcrProcessOptions", Create),
            set_max_lines: symbol!("OcrProcessOptionsSetMaxRecognitionLineCount", SetMaxLines),
            run: symbol!("RunOcrPipeline", Run),
            line_count: symbol!("GetOcrLineCount", LineCount),
            get_line: symbol!("GetOcrLine", GetLine),
            line_text: symbol!("GetOcrLineContent", LineText),
            release_result: symbol!("ReleaseOcrResult", Release),
            release_process: symbol!("ReleaseOcrProcessOptions", Release),
            release_pipeline: symbol!("ReleaseOcrPipeline", Release),
            release_init: symbol!("ReleaseOcrInitOptions", Release),
            _library: Some(library),
            _runtime: Some(runtime),
        })
    }
}

fn check(code: i32, operation: &str) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        Err(format!("OneOCR {operation} failed (code {code})"))
    }
}

pub struct Engine {
    api: Api,
    init: Handle,
    pipeline: Handle,
    process: Handle,
    pub directory: PathBuf,
}

impl Engine {
    pub fn discover() -> Result<Self, String> {
        let mut directories = Vec::new();
        let package_lookup = (|| -> windows::core::Result<()> {
            let manager = PackageManager::new()?;
            // Empty SID means the current user; no all-user enumeration or elevation.
            for package in manager.FindPackagesByUserSecurityId(&HSTRING::new())? {
                if package.Id()?.Name()? == "Microsoft.ScreenSketch" {
                    let root = PathBuf::from(package.InstalledLocation()?.Path()?.to_os_string());
                    directories.push(root.join("SnippingTool"));
                    directories.push(root);
                }
            }
            Ok(())
        })();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                directories.push(parent.join("oneocr"));
                directories.push(parent.to_path_buf());
            }
        }
        let mut errors = Vec::new();
        for directory in directories {
            if ["oneocr.dll", "oneocr.onemodel", "onnxruntime.dll"]
                .iter()
                .all(|file| directory.join(file).is_file())
            {
                match unsafe { Self::load(&directory) } {
                    Ok(engine) => return Ok(engine),
                    Err(error) => errors.push(error),
                }
            }
        }
        if let Err(error) = package_lookup {
            errors.push(format!("Snipping Tool lookup: {error}"));
        }
        if errors.is_empty() {
            errors.push("No accessible OneOCR files found. Install/update Snipping Tool in Microsoft Store, then restart MyOCR.".into());
        }
        Err(errors.join("\n"))
    }

    unsafe fn load(directory: &Path) -> Result<Self, String> {
        let api = Api::load(directory)?;
        // Construct the owner before any allocation, so partial failures also clean up.
        let mut engine = Self {
            api,
            init: null_mut(),
            pipeline: null_mut(),
            process: null_mut(),
            directory: directory.into(),
        };
        check(
            (engine.api.create_init)(&mut engine.init),
            "create init options",
        )?;
        if engine.init.is_null() {
            return Err("OneOCR returned null init options".into());
        }
        check(
            (engine.api.set_delay)(engine.init, 0),
            "disable delayed model loading",
        )?;
        let model = CString::new(
            directory
                .join("oneocr.onemodel")
                .to_string_lossy()
                .as_bytes(),
        )
        .map_err(|e| format!("Invalid OneOCR model path: {e}"))?;
        // Model initialization value published by the upstream OneOCR ABI research.
        let key = CString::new(r#"kj)TGtrK>f]b[Piow.gU+nC@s""""""4"#).unwrap();
        check(
            (engine.api.create_pipeline)(
                model.as_ptr(),
                key.as_ptr(),
                engine.init,
                &mut engine.pipeline,
            ),
            "load model",
        )?;
        if engine.pipeline.is_null() {
            return Err("OneOCR returned a null pipeline".into());
        }
        check(
            (engine.api.create_process)(&mut engine.process),
            "create process options",
        )?;
        if engine.process.is_null() {
            return Err("OneOCR returned null process options".into());
        }
        check(
            (engine.api.set_max_lines)(engine.process, 1000),
            "set max line count",
        )?;
        Ok(engine)
    }

    pub fn recognize(&self, pixels: &[u8], width: i32, height: i32) -> Result<String, String> {
        let pixels = opaque_pixels(pixels, width, height)?;
        let image = RawImage {
            format: 3,
            width,
            height,
            reserved: 0,
            stride: i64::from(width) * 4,
            data: pixels.as_ptr() as i64,
        };
        let mut result = ResultHandle {
            api: &self.api,
            handle: null_mut(),
        };
        unsafe {
            check(
                (self.api.run)(self.pipeline, &image, self.process, &mut result.handle),
                "recognize image",
            )?;
            if result.handle.is_null() {
                return Err("OneOCR returned a null result".into());
            }
            let mut count = 0;
            check(
                (self.api.line_count)(result.handle, &mut count),
                "read line count",
            )?;
            if !(0..=1000).contains(&count) {
                return Err(format!("OneOCR returned invalid line count: {count}"));
            }
            let mut lines = Vec::new();
            for index in 0..count {
                let mut line = null_mut();
                check(
                    (self.api.get_line)(result.handle, index, &mut line),
                    "read line",
                )?;
                if line.is_null() {
                    return Err("OneOCR returned a null line".into());
                }
                let mut text = null();
                check((self.api.line_text)(line, &mut text), "read text")?;
                if text.is_null() {
                    return Err("OneOCR returned null text".into());
                }
                lines.push(CStr::from_ptr(text).to_string_lossy().into_owned());
            }
            Ok(lines.join("\r\n").trim().to_owned())
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe {
            if !self.process.is_null() {
                (self.api.release_process)(self.process);
            }
            if !self.pipeline.is_null() {
                (self.api.release_pipeline)(self.pipeline);
            }
            if !self.init.is_null() {
                (self.api.release_init)(self.init);
            }
        }
    }
}

struct ResultHandle<'a> {
    api: &'a Api,
    handle: Handle,
}
impl Drop for ResultHandle<'_> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                (self.api.release_result)(self.handle);
            }
        }
    }
}

fn opaque_pixels(pixels: &[u8], width: i32, height: i32) -> Result<Vec<u8>, String> {
    let length = usize::try_from(width)
        .ok()
        .zip(usize::try_from(height).ok())
        .and_then(|(w, h)| w.checked_mul(h)?.checked_mul(4));
    if width <= 0 || height <= 0 || length != Some(pixels.len()) {
        return Err("Invalid screenshot dimensions or pixel buffer".into());
    }
    // Native DIB data is BGRA and has undefined (often zero) alpha.
    // OneOCR takes four-channel pixels; convert to RGBA and force opaque alpha.
    Ok(pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[2], p[1], p[0], 255])
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dib_pixels_have_correct_color_and_opaque_alpha() {
        assert_eq!(
            opaque_pixels(&[10, 20, 30, 0, 40, 50, 60, 7], 2, 1).unwrap(),
            vec![30, 20, 10, 255, 60, 50, 40, 255]
        );
    }
    #[test]
    fn invalid_or_overflowing_image_dimensions_are_rejected() {
        assert!(opaque_pixels(&[], 0, 2).is_err());
        assert!(opaque_pixels(&[], -1, 2).is_err());
        assert!(opaque_pixels(&[0; 4], 2, 2).is_err());
        assert!(opaque_pixels(&[], i32::MAX, i32::MAX).is_err());
    }
    #[test]
    fn absent_dll_is_a_recoverable_error() {
        assert!(
            unsafe { Api::load(&std::env::temp_dir().join("myocr-nonexistent-oneocr")) }.is_err()
        );
    }
    #[test]
    fn image_abi_matches_native_32_byte_layout() {
        assert_eq!(std::mem::size_of::<RawImage>(), 32);
        assert_eq!(std::mem::offset_of!(RawImage, stride), 16);
        assert_eq!(std::mem::offset_of!(RawImage, data), 24);
    }

    thread_local! {
        static RELEASES: std::cell::RefCell<Vec<&'static str>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    unsafe extern "system" fn create(_: *mut Handle) -> i32 {
        0
    }
    unsafe extern "system" fn delay(_: Handle, _: u8) -> i32 {
        0
    }
    unsafe extern "system" fn pipeline(
        _: *const c_char,
        _: *const c_char,
        _: Handle,
        _: *mut Handle,
    ) -> i32 {
        0
    }
    unsafe extern "system" fn max_lines(_: Handle, _: i32) -> i32 {
        0
    }
    unsafe extern "system" fn run(
        _: Handle,
        image: *const RawImage,
        _: Handle,
        result: *mut Handle,
    ) -> i32 {
        let image = &*image;
        assert_eq!((image.width, image.height, image.stride), (1, 1, 4));
        assert_eq!(
            std::slice::from_raw_parts(image.data as *const u8, 4),
            &[30, 20, 10, 255]
        );
        *result = 1usize as Handle;
        0
    }
    unsafe extern "system" fn count(_: Handle, count: *mut i64) -> i32 {
        *count = 1;
        0
    }
    unsafe extern "system" fn line(_: Handle, _: i64, line: *mut Handle) -> i32 {
        *line = 2usize as Handle;
        0
    }
    unsafe extern "system" fn text(_: Handle, text: *mut *const c_char) -> i32 {
        *text = c"中文 English".as_ptr();
        0
    }
    unsafe extern "system" fn release_result(_: Handle) {
        RELEASES.with_borrow_mut(|r| r.push("result"));
    }
    unsafe extern "system" fn release_process(_: Handle) {
        RELEASES.with_borrow_mut(|r| r.push("process"));
    }
    unsafe extern "system" fn release_pipeline(_: Handle) {
        RELEASES.with_borrow_mut(|r| r.push("pipeline"));
    }
    unsafe extern "system" fn release_init(_: Handle) {
        RELEASES.with_borrow_mut(|r| r.push("init"));
    }

    fn mock_engine() -> Engine {
        Engine {
            api: Api {
                create_init: create,
                set_delay: delay,
                create_pipeline: pipeline,
                create_process: create,
                set_max_lines: max_lines,
                run,
                line_count: count,
                get_line: line,
                line_text: text,
                release_result,
                release_process,
                release_pipeline,
                release_init,
                _library: None,
                _runtime: None,
            },
            init: 1usize as Handle,
            pipeline: 2usize as Handle,
            process: 3usize as Handle,
            directory: PathBuf::new(),
        }
    }

    #[test]
    fn native_result_text_and_lifetimes_are_preserved() {
        RELEASES.with_borrow_mut(Vec::clear);
        {
            let engine = mock_engine();
            assert_eq!(
                engine.recognize(&[10, 20, 30, 0], 1, 1).unwrap(),
                "中文 English"
            );
            RELEASES.with_borrow(|r| assert_eq!(r, &["result"]));
        }
        RELEASES.with_borrow(|r| assert_eq!(r, &["result", "process", "pipeline", "init"]));
    }

    #[test]
    fn native_error_releases_result_and_partial_initialization() {
        unsafe extern "system" fn failed_count(_: Handle, _: *mut i64) -> i32 {
            42
        }
        RELEASES.with_borrow_mut(Vec::clear);
        {
            let mut engine = mock_engine();
            engine.api.line_count = failed_count;
            engine.process = null_mut(); // Initialization did not reach this allocation.
            assert!(engine
                .recognize(&[10, 20, 30, 0], 1, 1)
                .unwrap_err()
                .contains("42"));
        }
        RELEASES.with_borrow(|r| assert_eq!(r, &["result", "pipeline", "init"]));
    }
}
