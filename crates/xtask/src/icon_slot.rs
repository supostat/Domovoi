use crate::icon_artwork::IconArtwork;

const SMALL_ARTWORK_PIXELS: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IconSlot {
    pub(crate) file_name: &'static str,
    pub(crate) pixels: u32,
}

impl IconSlot {
    pub(crate) fn artwork(self) -> IconArtwork {
        if self.pixels <= SMALL_ARTWORK_PIXELS {
            IconArtwork::Small
        } else {
            IconArtwork::Full
        }
    }
}

pub(crate) const ICON_SLOTS: [IconSlot; 10] = [
    IconSlot {
        file_name: "icon_16x16.png",
        pixels: 16,
    },
    IconSlot {
        file_name: "icon_16x16@2x.png",
        pixels: 32,
    },
    IconSlot {
        file_name: "icon_32x32.png",
        pixels: 32,
    },
    IconSlot {
        file_name: "icon_32x32@2x.png",
        pixels: 64,
    },
    IconSlot {
        file_name: "icon_128x128.png",
        pixels: 128,
    },
    IconSlot {
        file_name: "icon_128x128@2x.png",
        pixels: 256,
    },
    IconSlot {
        file_name: "icon_256x256.png",
        pixels: 256,
    },
    IconSlot {
        file_name: "icon_256x256@2x.png",
        pixels: 512,
    },
    IconSlot {
        file_name: "icon_512x512.png",
        pixels: 512,
    },
    IconSlot {
        file_name: "icon_512x512@2x.png",
        pixels: 1024,
    },
];
