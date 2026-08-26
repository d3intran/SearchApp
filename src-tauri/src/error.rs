use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Excel 解析失败: {0}")]
    Excel(#[from] calamine::Error),

    #[error("PDF 解析失败: {0}")]
    Pdf(#[from] pdf::error::PdfError),

    #[error("网络请求失败: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Excel 写入失败: {0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError),

    #[error("Word 解压失败: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Word 解析失败: {0}")]
    Word(String),

    #[error("XML 解析失败: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("不支持的文件格式: .{0}")]
    UnsupportedFormat(String),

    #[error("配置路径错误: {0}")]
    ConfigPath(String),

    #[error("状态路径错误: {0}")]
    StatePath(String),

    #[error("{0}")]
    Custom(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
