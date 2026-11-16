use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString, assets};

assets::icon_assets!(
    ExtraIcons,
    [
        Clock,
        CreditCard,
        FileChartColumn,
        FingerprintPattern,
        LogOut,
        Pencil,
        Phone,
        ReceiptText,
        ShieldCheck,
        UserCheck,
        UserX,
        Users,
        Utensils,
        Wallet,
    ]
);

/// Fuente de activos compuesta para la aplicación de escritorio.
///
/// Combina los iconos predeterminados de GPUI Kit (`assets::Assets`) con los
/// iconos adicionales seleccionados del catálogo Lucide mediante `icon_assets!`.
#[derive(Clone, Copy, Debug, Default)]
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
