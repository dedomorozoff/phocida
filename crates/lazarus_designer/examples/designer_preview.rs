//! A preview window for the form designer skeleton: renders a sample form
//! from its LFM text next to the component palette. Run with:
//! `cargo run -p lazarus_designer --example designer_preview`

use gpui::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window,
    WindowBounds, WindowOptions, div, px, rgb, size,
};
use gpui_platform::application;

use lazarus_designer::{FormCanvas, palette_components, parse_lfm};

const SAMPLE_LFM: &str = r#"object Form1: TForm1
  Left = 0
  Top = 0
  Width = 480
  Height = 360
  Caption = 'Form1'
  object Button1: TButton
    Left = 24
    Top = 24
    Width = 100
    Height = 30
    Caption = 'OK'
  end
  object Label1: TLabel
    Left = 24
    Top = 70
    Width = 120
    Height = 20
    Caption = 'Name:'
  end
  object Edit1: TEdit
    Left = 24
    Top = 100
    Width = 180
    Height = 24
    Text = 'hello'
  end
  object Panel1: TPanel
    Left = 24
    Top = 150
    Width = 420
    Height = 120
  end
end"#;

struct Preview {
    canvas: gpui::Entity<FormCanvas>,
}

impl Render for Preview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(0x1a1a1e))
            .child(div().flex_1().size_full().child(self.canvas.clone()))
            .child(
                div()
                    .w(px(170.0))
                    .h_full()
                    .bg(rgb(0x222227))
                    .border_l_1()
                    .border_color(rgb(0x333338))
                    .p_2()
                    .child(div().flex().flex_col().gap_1().children(
                        palette_components().into_iter().map(|descriptor| {
                            div()
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(0x2c2c31))
                                .text_color(rgb(0xd8d8d8))
                                .text_size(px(12.0))
                                .child(format!("{} ({})", descriptor.label, descriptor.class))
                        }),
                    )),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let doc = parse_lfm(SAMPLE_LFM).expect("sample lfm parses");
        let canvas =
            cx.new(|_| FormCanvas::new(lazarus_designer::components_of(&doc), doc.root.bounds));
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(880.), px(560.)),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Preview { canvas }),
        )
        .unwrap();
        cx.activate(true);
    });
}
