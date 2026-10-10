//! Describe the painted surface to the compositor instead of blurring its bounding box.
use std::collections::HashMap;

use smithay_client_toolkit::{
    compositor::{CompositorState, Region},
    reexports::protocols::ext::background_effect::v1::client::{
        ext_background_effect_manager_v1::{self, ExtBackgroundEffectManagerV1},
        ext_background_effect_surface_v1::{self, ExtBackgroundEffectSurfaceV1},
    },
};
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum, globals::GlobalList, protocol::wl_surface,
};

use crate::LiftApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BlurRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Both SHM pixels and protocol regions use surface coordinates: Lift uses buffer scale 1.
/// Merge matching runs on adjacent rows to keep rounded corners to a few rectangles.
pub(crate) fn alpha_rects(pixels: &[u8], width: u32, height: u32) -> Vec<BlurRect> {
    let mut rects: Vec<BlurRect> = Vec::new();
    let mut previous: HashMap<(i32, i32), usize> = HashMap::new();
    let mut current = HashMap::new();
    for (y, row) in pixels
        .chunks_exact(width as usize * 4)
        .take(height as usize)
        .enumerate()
    {
        current.clear();
        let mut x = 0;
        while x < width as usize {
            if row[x * 4 + 3] == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < width as usize && row[x * 4 + 3] != 0 {
                x += 1;
            }
            let run = (start as i32, (x - start) as i32);
            let index = if let Some(&index) = previous.get(&run) {
                rects[index].height += 1;
                index
            } else {
                let index = rects.len();
                rects.push(BlurRect {
                    x: run.0,
                    y: y as i32,
                    width: run.1,
                    height: 1,
                });
                index
            };
            current.insert(run, index);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    rects
}

pub(crate) struct BackgroundBlur {
    _manager: Option<ExtBackgroundEffectManagerV1>,
    surface: Option<ExtBackgroundEffectSurfaceV1>,
    supported: bool,
    last_region: Option<Vec<BlurRect>>,
}

impl BackgroundBlur {
    pub fn bind(
        globals: &GlobalList,
        qh: &QueueHandle<LiftApp>,
        surface: &wl_surface::WlSurface,
    ) -> Self {
        // Older compositors may not advertise this optional extension.
        let manager: Option<ExtBackgroundEffectManagerV1> = globals.bind(qh, 1..=1, ()).ok();
        let surface = manager
            .as_ref()
            .map(|manager| manager.get_background_effect(surface, qh, ()));
        Self {
            _manager: manager,
            surface,
            supported: false,
            last_region: None,
        }
    }

    pub fn update(
        &mut self,
        compositor: &CompositorState,
        pixels: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let Some(surface) = self.surface.as_ref().filter(|_| self.supported) else {
            return Ok(());
        };
        let rects = alpha_rects(pixels, width, height);
        if self.last_region.as_ref() == Some(&rects) {
            return Ok(());
        }
        let region = Region::new(compositor).map_err(|err| format!("blur region: {err}"))?;
        for rect in &rects {
            region.add(rect.x, rect.y, rect.width, rect.height);
        }
        // The region is copied by this request, then applied with the new buffer's commit.
        surface.set_blur_region(Some(region.wl_region()));
        self.last_region = Some(rects);
        Ok(())
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for LiftApp {
    fn event(
        app: &mut Self,
        _: &ExtBackgroundEffectManagerV1,
        event: ext_background_effect_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_background_effect_manager_v1::Event::Capabilities { flags } = event {
            let bits = match flags {
                WEnum::Value(flags) => flags.bits(),
                WEnum::Unknown(bits) => bits,
            };
            app.blur.supported =
                bits & ext_background_effect_manager_v1::Capability::Blur.bits() != 0;
            app.blur.last_region = None;
            app.mark_redraw();
        }
    }
}

impl Dispatch<ExtBackgroundEffectSurfaceV1, ()> for LiftApp {
    fn event(
        _: &mut Self,
        _: &ExtBackgroundEffectSurfaceV1,
        _: ext_background_effect_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // This interface has no events.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_adjacent_runs_without_filling_holes_or_gaps() {
        let mask = [
            [0, 1, 255, 255, 1, 0],
            [0, 1, 255, 255, 1, 0],
            [255, 255, 0, 0, 255, 255],
            [255, 255, 0, 0, 255, 255],
            [0, 0, 0, 0, 0, 0],
            [0, 1, 255, 255, 1, 0],
        ];
        let pixels: Vec<_> = mask.iter().flatten().flat_map(|&a| [0, 0, 0, a]).collect();
        let rects = alpha_rects(&pixels, 6, 6);
        assert_eq!(rects.len(), 4);
        assert_eq!(
            rects[0],
            BlurRect {
                x: 1,
                y: 0,
                width: 4,
                height: 2
            }
        );
        for (y, row) in mask.iter().enumerate() {
            for (x, &alpha) in row.iter().enumerate() {
                let count = rects
                    .iter()
                    .filter(|r| {
                        x as i32 >= r.x
                            && (x as i32) < r.x + r.width
                            && y as i32 >= r.y
                            && (y as i32) < r.y + r.height
                    })
                    .count();
                assert_eq!(count, usize::from(alpha != 0), "pixel {x},{y}");
            }
        }
    }

    #[test]
    fn transparent_buffer_has_no_blur_rectangles() {
        assert!(alpha_rects(&[0; 24], 3, 2).is_empty());
    }
}
