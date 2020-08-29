use druid::kurbo::{Line, Point, Rect, Size};
use druid::widget::{Axis, CrossAxisAlignment};
use druid::{AppLauncher, Color, Data, Lens, Widget, WindowDesc};
use druid_table::{BandScale, DrawableAxis, F64Range, LinearScale, LogIdx, Mark, MarkId, MarkShape, Vis, Visualization, VisEvent};
use im::Vector;
use itertools::Itertools;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Display;
use std::hash::Hash;

#[macro_use]
extern crate im;

// Working from
// https://vega.github.io/vega/examples/bar-chart/

fn main_widget()->impl Widget<TopLevel>{
    Vis::new(MyBarChart{
        scales: None
    } )
}

fn main() {
    let main_window = WindowDesc::new(main_widget)
        .title("Visualisation")
        .window_size((800.0, 500.0));

    // create the initial app state
    let initial_state = TopLevel {
        records: vector![
            ("A".into(), 28),
            ("B".into(), 55),
            ("C".into(), 43),
            ("D".into(), 91),
            ("E".into(), 81),
            ("F".into(), 53),
            ("G".into(), 19),
            ("H".into(), 87)
        ],
    };

    // start the application
    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(initial_state)
        .expect("Failed to launch application");
}

type CatCount = (String, u32);

#[derive(Clone, Data, Lens)]
struct TopLevel {
    records: Vector<CatCount>,
}

struct MyBarChart{
    scales: Option<(BandScale<String>, LinearScale<u32>)>
}

impl Visualization for MyBarChart {
    type Input = TopLevel;
    type State = Option<CatCount>;

    fn layout(&mut self, data: &Self::Input, size: Size) {
        self.scales = Some(
            (
            BandScale::new(
                F64Range(30.0, size.width),
                &mut data.records.iter().map(|x| (x.0).clone()),
                0.05,
            ),
            LinearScale::new(
                F64Range(30.0, size.height - 10.0),
                &mut data.records.iter().map(|x| (x.1).clone()),
                true,
                None,
                true,
            ),
        ));
    }

    fn event(&mut self, data: &mut Self::Input, tooltip_item: &mut Option<CatCount>, event: &VisEvent) {
        match event {
            VisEvent::MouseEnter(MarkId::Datum { idx })=>*tooltip_item = data.records.get(idx.0).cloned(),
            VisEvent::MouseOut(_)=> *tooltip_item = None,
            _=>()
        };
    }

    fn state_marks(&self, data: &Self::Input, tooltip_item: &Option<CatCount>) -> Vec<Mark> {
        let mut marks = Vec::new();
        if let Some(tt) = tooltip_item {
            log::info!("TT: {:?}", tt);
            if let Some((x, y)) = &self.scales {
                log::info!("push: {:?}", tt);
                marks.push(Mark::new(MarkId::Unknown, MarkShape::Text {
                    txt: tt.1.to_string(),
                    font_fam: Default::default(),
                    size: 12.0,
                    point: Point::new(
                        x.range_val(&tt.0).mid(),
                        y.range_val(&tt.1) - 2.0
                    )
                }, Color::rgb8(0xD0, 0xD0, 0xD0), None));
            }
        }
        marks
    }

    fn data_marks(&self, data: &Self::Input) -> Vec<Mark> {
        if let Some((x, y)) = &self.scales {
            data.records
                .iter()
                .enumerate()
                .map(|(idx, (cat, amount))| {
                    let xr = x.range_val(cat);
                    let r = Rect::new(xr.0, y.range.0, xr.1, y.range_val(amount));
                    Mark::new(
                        MarkId::Datum { idx: LogIdx(idx) },
                        MarkShape::Rect(r),
                        Color::rgb8(0x46, 0x82, 0xb4),
                        Some(Color::rgb8(0xFF, 0, 0)),
                    )
                })
                .collect()
        }else{
            vec![]
        }
    }

    fn drawable_axes(&self) -> Vec<DrawableAxis> {
        if let Some((x, y)) = &self.scales {
            vec![x.make_axis(), y.make_axis()]
        }else{
            vec![]
        }
    }
}