use {
    crate::ifs::wl_output::{
        TF_180, TF_270, TF_90, TF_FLIPPED, TF_FLIPPED_180, TF_FLIPPED_270, TF_FLIPPED_90, TF_NORMAL,
    },
    jay_config::video::{
        Transform,
        Transform::{
            Flip, FlipRotate180, FlipRotate270, FlipRotate90, None, Rotate180, Rotate270, Rotate90,
        },
    },
};

pub trait TransformExt: Sized {
    fn maybe_swap<T>(self, args: (T, T)) -> (T, T);

    fn to_wl(self) -> i32;

    fn from_wl(wl: i32) -> Option<Self>;

    fn apply_point(self, width: i32, height: i32, point: (i32, i32)) -> (i32, i32);

    fn inverse(self) -> Self;
}

impl TransformExt for Transform {
    fn maybe_swap<T>(self, (left, right): (T, T)) -> (T, T) {
        match self {
            None | Rotate180 | Flip | FlipRotate180 => (left, right),
            Rotate90 | Rotate270 | FlipRotate90 | FlipRotate270 => (right, left),
        }
    }

    fn to_wl(self) -> i32 {
        match self {
            None => TF_NORMAL,
            Rotate90 => TF_90,
            Rotate180 => TF_180,
            Rotate270 => TF_270,
            Flip => TF_FLIPPED,
            FlipRotate90 => TF_FLIPPED_90,
            FlipRotate180 => TF_FLIPPED_180,
            FlipRotate270 => TF_FLIPPED_270,
        }
    }

    fn from_wl(wl: i32) -> Option<Self> {
        let tf = match wl {
            TF_NORMAL => None,
            TF_90 => Rotate90,
            TF_180 => Rotate180,
            TF_270 => Rotate270,
            TF_FLIPPED => Flip,
            TF_FLIPPED_90 => FlipRotate90,
            TF_FLIPPED_180 => FlipRotate180,
            TF_FLIPPED_270 => FlipRotate270,
            _ => return Option::None,
        };
        Some(tf)
    }

    fn apply_point(self, width: i32, height: i32, (x, y): (i32, i32)) -> (i32, i32) {
        match self {
            None => (x, y),
            Rotate90 => (y, height - x),
            Rotate180 => (width - x, height - y),
            Rotate270 => (width - y, x),
            Flip => (width - x, y),
            FlipRotate90 => (y, x),
            FlipRotate180 => (x, height - y),
            FlipRotate270 => (width - y, height - x),
        }
    }

    fn inverse(self) -> Self {
        match self {
            Rotate90 => Rotate270,
            Rotate270 => Rotate90,
            _ => self,
        }
    }
}
