//! Native first-paint diagnostic: the production ArticleViewport, three tiny
//! synthetic images, delayed metadata and finite frame-only row updates.
//! No transport, profile, clipboard, snapshot or continuous idle polling.
use slint::{
    ComponentHandle, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode, VecModel,
};
use std::{cell::Cell, error::Error, rc::Rc, time::Duration};

slint::slint! {
    import { ArticleViewport, ArticleTile, InlinePicture } from "../src/bin/slint_research/article.slint";
    import { Button } from "std-widgets.slint";
    export component PictureRepro inherits Window {
        title: "PTTStella · native picture first paint";
        preferred-width:1000px;preferred-height:690px;background:#101318;
        in property <[InlinePicture]> pictures;
        in property <[ArticleTile]> tiles;
        in property <string> status;
        callback load;
        callback reset;
        HorizontalLayout {
            x:12px;y:8px;height:36px;width:500px;
            Button {text:"Load three pictures";clicked=>{root.load();}}
            Button {text:"Reset";clicked=>{root.reset();}}
            Text {text:root.status;color:white;vertical-alignment:center;}
        }
        ArticleViewport {
            x:12px;y:60px;width:parent.width - 24px;height:400px;
            line-width:640px;line-height:16px;body-height:372px;document-height:1000px;
            tiles:root.tiles;inline-pictures:root.pictures;inline-enabled:true;
            native-input:false;read-only:true;
        }
    }
}

struct Owner {
    ui: slint::Weak<PictureRepro>,
    pictures: Rc<VecModel<InlinePicture>>,
    metadata: Timer,
    animation: Timer,
    stage: Cell<usize>,
    frame: Cell<u8>,
}
fn image(blue: bool, split: bool) -> slint::Image {
    let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(64, 32);
    for (i, p) in pixels.make_mut_slice().iter_mut().enumerate() {
        *p = if blue || split && i % 64 >= 32 {
            Rgba8Pixel::new(0, 0, 255, 255)
        } else {
            Rgba8Pixel::new(255, 0, 0, 255)
        };
    }
    slint::Image::from_rgba8(pixels)
}
impl Owner {
    fn reset(&self) {
        self.metadata.stop();
        self.animation.stop();
        self.stage.set(0);
        self.frame.set(0);
        self.pictures.set_vec(Vec::new());
        if let Some(ui) = self.ui.upgrade() {
            ui.set_status("Ready".into());
            ui.set_tiles(ModelRc::default());
        }
    }
    fn load(self: &Rc<Self>) {
        self.reset();
        for key in 0..3 {
            self.pictures.push(InlinePicture {
                index: key,
                key,
                stamp: "repro".into(),
                x: 162.,
                y: 36. + key as f32 * 76.,
                width: 440.,
                height: 36.,
                image_y: 0.,
                image_height: 36.,
                source_y: 0,
                source_height: 1,
                source_width: 1,
                image: slint::Image::default(),
                status: "Loading".into(),
            });
        }
        self.publish_tiles();
        let weak = Rc::downgrade(self);
        self.metadata
            .start(TimerMode::Repeated, Duration::from_millis(10), move || {
                if let Some(owner) = weak.upgrade() {
                    owner.metadata();
                }
            });
    }
    fn metadata(self: &Rc<Self>) {
        let key = self.stage.get();
        if key >= 3 {
            self.metadata.stop();
            return;
        }
        if let Some(mut p) = self.pictures.row_data(key) {
            p.width = 64.;
            p.height = 32.;
            p.image_height = 32.;
            p.source_width = 64;
            p.source_height = 32;
            p.image = image(key == 2, key == 0);
            p.status = "".into();
            self.pictures.set_row_data(key, p);
        }
        for row in key + 1..3 {
            if let Some(mut p) = self.pictures.row_data(row) {
                p.y -= 4.;
                self.pictures.set_row_data(row, p);
            }
        }
        self.stage.set(key + 1);
        self.publish_tiles();
        eprintln!("metadata stage={}", key + 1);
        if key == 2 {
            self.metadata.stop();
            if let Some(ui) = self.ui.upgrade() {
                ui.set_status("3 ready; 12 finite GIF steps".into());
            }
            let weak = Rc::downgrade(self);
            self.animation
                .start(TimerMode::Repeated, Duration::from_millis(150), move || {
                    if let Some(owner) = weak.upgrade() {
                        let frame = owner.frame.get() + 1;
                        owner.frame.set(frame);
                        if let Some(mut p) = owner.pictures.row_data(1) {
                            p.image = image(frame % 2 == 1, false);
                            owner.pictures.set_row_data(1, p);
                        }
                        if frame >= 12 {
                            owner.animation.stop();
                            if let Some(ui) = owner.ui.upgrade() {
                                ui.set_status("3 ready; animation stopped".into());
                            }
                        }
                    }
                });
        }
    }
    fn publish_tiles(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let mut y = 0.;
        let mut tiles = Vec::new();
        for row in 0..18 {
            let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(640, 16);
            for (i, p) in pixels.make_mut_slice().iter_mut().enumerate() {
                let x = i % 640;
                *p = if i / 640 == 5 && x < (row + 1) * 24 {
                    Rgba8Pixel::new(180, 180, 180, 255)
                } else {
                    Rgba8Pixel::new(0, 0, 0, 255)
                };
            }
            tiles.push(ArticleTile {
                image: slint::Image::from_rgba8(pixels),
                y,
                height: 16.,
            });
            y += 16.;
            if [1, 3, 5].contains(&row) {
                let key = (row - 1) / 2;
                if let Some(p) = self.pictures.row_data(key) {
                    y += p.height + 8.;
                }
            }
        }
        ui.set_tiles(ModelRc::new(VecModel::from(tiles)));
    }
}
/// Independent decoder check for the fixed 1000x718 native inline fixture.
/// This consumes saved OS captures, not Slint's toolkit snapshot renderer.
fn inspect_native_capture(path: &std::path::Path) -> Result<serde_json::Value, Box<dyn Error>> {
    let dimensions = image::ImageReader::open(path)?
        .with_guessed_format()?
        .into_dimensions()?;
    if dimensions != (1000, 718) {
        return Err(
            std::io::Error::other("capture must use the fixed 1000x718 native fixture").into(),
        );
    }
    let pixels = image::open(path)?.into_rgba8();
    let mut areas = Vec::new();
    for (name, x, y, width, height, kind) in [
        ("png-left", 178, 186, 24, 24, 0),
        ("png-right", 210, 186, 24, 24, 1),
        ("gif", 178, 258, 56, 24, 2),
        ("once", 178, 330, 56, 24, 2),
    ] {
        let (mut red, mut blue) = (0, 0);
        for row in y..y + height {
            for col in x..x + width {
                let p = pixels.get_pixel(col, row).0;
                red += u32::from(p[0] > 180 && p[1] < 110 && p[2] < 110);
                blue += u32::from(p[2] > 180 && p[0] < 110 && p[1] < 110);
            }
        }
        let matched = match kind {
            0 => red,
            1 => blue,
            _ => red.max(blue),
        };
        let total = width * height;
        let passed = matched * 100 >= total * 95;
        areas
            .push(serde_json::json!({"name":name,"matched":matched,"total":total,"passed":passed}));
    }
    let passed = areas.iter().all(|a| a["passed"] == true);
    let result = serde_json::json!({"path":path,"dimensions":dimensions,"areas":areas,"passed":passed,"decoder":"image crate; independent of CUA jpeg-js/pngjs"});
    if !passed {
        return Err(std::io::Error::other(result.to_string()).into());
    }
    Ok(result)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    if let Some(flag) = arguments.next() {
        if flag != "--check-native-inline-capture" {
            return Err(std::io::Error::other("unknown picture diagnostic argument").into());
        }
        let path = arguments
            .next()
            .ok_or_else(|| std::io::Error::other("missing capture path"))?;
        if arguments.next().is_some() {
            return Err(std::io::Error::other("unexpected picture diagnostic argument").into());
        }
        println!("{}", inspect_native_capture(std::path::Path::new(&path))?);
        return Ok(());
    }
    let ui = PictureRepro::new()?;
    let owner = Rc::new(Owner {
        ui: ui.as_weak(),
        pictures: Rc::new(VecModel::default()),
        metadata: Timer::default(),
        animation: Timer::default(),
        stage: Cell::new(0),
        frame: Cell::new(0),
    });
    ui.set_pictures(ModelRc::from(owner.pictures.clone()));
    ui.on_load({
        let owner = owner.clone();
        move || owner.load()
    });
    ui.on_reset({
        let owner = owner.clone();
        move || owner.reset()
    });
    owner.reset();
    ui.run()?;
    owner.reset();
    Ok(())
}
