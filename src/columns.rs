use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use crate::axis_measure::{AxisPair, LogIdx};
use crate::data::SortDirection::Ascending;
use crate::data::{RemapDetails, SortDirection, SortSpec};
use crate::selection::SingleCell;
use crate::{CellsDelegate, IndexedData, IndexedItems, Remap, RemapSpec, Remapper, TableAxis};
use druid::im::Vector;
use druid::kurbo::{Line, PathEl};
use druid::piet::{FontFamily, Text, TextLayoutBuilder};
use druid::widget::prelude::*;
use druid::widget::TextBox;
use druid::{theme, ArcStr, Color, Data, Env, KeyOrValue, Lens, PaintCtx, Point, WidgetExt};
use std::cmp::Ordering;
use std::fmt;
use std::fmt::{Debug, Formatter};

pub trait EditorFactory<RowData> {
    fn make_editor(&mut self, ctx: &CellCtx) -> Option<Box<dyn Widget<RowData>>>;
}

pub trait CellDelegate<RowData>:
    CellRender<RowData> + DataCompare<RowData> + EditorFactory<RowData>
{
}

impl<T> CellRender<T> for Box<dyn CellDelegate<T>> {
    fn init(&mut self, ctx: &mut PaintCtx, env: &Env) {
        self.deref_mut().init(ctx, env)
    }
    fn paint(&self, ctx: &mut PaintCtx, cell: &CellCtx, data: &T, env: &Env) {
        self.deref().paint(ctx, cell, data, env);
    }
}

impl<RowData> EditorFactory<RowData> for Box<dyn CellDelegate<RowData>> {
    fn make_editor(&mut self, ctx: &CellCtx) -> Option<Box<dyn Widget<RowData>>> {
        self.deref_mut().make_editor(ctx)
    }
}

impl<T> DataCompare<T> for Box<dyn CellDelegate<T>> {
    fn compare(&self, a: &T, b: &T) -> Ordering {
        self.deref().compare(a, b)
    }
}

impl<RowData, T> CellDelegate<RowData> for T where
    T: CellRender<RowData> + DataCompare<RowData> + EditorFactory<RowData>
{
}

// Todo change to boxed header delegate?
impl<T> CellRender<T> for Box<dyn CellRender<T>> {
    fn init(&mut self, ctx: &mut PaintCtx, env: &Env) {
        self.deref_mut().init(ctx, env)
    }
    fn paint(&self, ctx: &mut PaintCtx, cell: &CellCtx, data: &T, env: &Env) {
        self.deref().paint(ctx, cell, data, env);
    }
}

#[derive(Debug)]
pub enum CellCtx<'a> {
    Absent,
    Cell(&'a SingleCell),
    Header(&'a TableAxis, LogIdx, Option<&'a SortSpec>),
}

pub trait CellRender<T> {
    fn init(&mut self, ctx: &mut PaintCtx, env: &Env); // Use to cache resources like fonts
    fn paint(&self, ctx: &mut PaintCtx, cell: &CellCtx, data: &T, env: &Env);
}

impl<T, CR: CellRender<T>> CellRender<T> for Vec<CR> {
    fn init(&mut self, ctx: &mut PaintCtx, env: &Env) {
        for col in self {
            col.init(ctx, env)
        }
    }

    fn paint(&self, ctx: &mut PaintCtx, cell: &CellCtx, data: &T, env: &Env) {
        if let CellCtx::Cell(SingleCell {
            log: AxisPair { col, .. },
            ..
        }) = cell
        {
            if let Some(cell_render) = self.get(col.0) {
                cell_render.paint(ctx, cell, data, env)
            }
        }
    }
}

impl<T, EF: EditorFactory<T>> EditorFactory<T> for Vec<EF> {
    fn make_editor(&mut self, cell: &CellCtx) -> Option<Box<dyn Widget<T>>> {
        if let CellCtx::Cell(SingleCell {
            log: AxisPair { col, .. },
            ..
        }) = cell
        {
            if let Some(ef) = self.get_mut(col.0) {
                return ef.make_editor(cell);
            }
        }
        None
    }
}

#[derive(Clone)]
pub struct Wrapped<T, U, W, I> {
    inner: I,
    wrapper: W,
    phantom_u: PhantomData<U>,
    phantom_t: PhantomData<T>,
}

pub struct LensWrapped<T, U, W, I>(Wrapped<T, U, W, I>)
where
    W: Lens<T, U>;

#[derive(Clone)]
pub struct FuncWrapped<T, U, W, I>(Wrapped<T, U, W, I>)
where
    W: Fn(&T) -> U;

impl<T, U, W, I> Wrapped<T, U, W, I> {
    fn new(inner: I, wrapper: W) -> Wrapped<T, U, W, I> {
        Wrapped {
            inner,
            wrapper,
            phantom_u: PhantomData,
            phantom_t: PhantomData,
        }
    }
}

pub trait CellRenderExt<T: Data>: CellRender<T> + Sized + 'static {
    fn lens<S: Data, L: Lens<S, T>>(self, lens: L) -> LensWrapped<S, T, L, Self> {
        LensWrapped(Wrapped::new(self, lens))
    }

    fn on_result_of<S: Data, F: Fn(&S) -> T>(self, f: F) -> FuncWrapped<S, T, F, Self> {
        FuncWrapped(Wrapped::new(self, f))
    }
}

impl<T: Data, CR: CellRender<T> + 'static> CellRenderExt<T> for CR {}

pub trait DataCompare<Item> {
    fn compare(&self, a: &Item, b: &Item) -> Ordering;
}

impl<T, U, L, CR> CellRender<T> for LensWrapped<T, U, L, CR>
where
    T: Data,
    U: Data,
    L: Lens<T, U>,
    CR: CellRender<U>,
{
    fn init(&mut self, ctx: &mut PaintCtx, env: &Env) {
        self.0.inner.init(ctx, env)
    }

    fn paint(&self, ctx: &mut PaintCtx, cell: &CellCtx, data: &T, env: &Env) {
        let inner = &self.0.inner;
        self.0.wrapper.with(data, |inner_data| {
            inner.paint(ctx, cell, inner_data, env);
        })
    }
}

impl<T, U, L, DC> DataCompare<T> for LensWrapped<T, U, L, DC>
where
    T: Data,
    U: Data,
    L: Lens<T, U>,
    DC: DataCompare<U>,
{
    fn compare(&self, a: &T, b: &T) -> Ordering {
        self.0.wrapper.with(a, |a| {
            self.0.wrapper.with(b, |b| self.0.inner.compare(a, b))
        })
    }
}

impl<T, U, L, EF> EditorFactory<T> for LensWrapped<T, U, L, EF>
where
    T: Data,
    U: Data,
    L: Lens<T, U> + Clone + 'static,
    EF: EditorFactory<U>,
{
    fn make_editor(&mut self, ctx: &CellCtx) -> Option<Box<dyn Widget<T>>> {
        // TODO work out if we can avoid a chain of boxing...
        if let Some(widget) = self.0.inner.make_editor(ctx) {
            Some(Box::new(widget.lens(self.0.wrapper.clone())))
        } else {
            None
        }
    }
}

impl<T, U, F, CR> CellRender<T> for FuncWrapped<T, U, F, CR>
where
    T: Data,
    U: Data,
    F: Fn(&T) -> U,
    CR: CellRender<U>,
{
    fn init(&mut self, ctx: &mut PaintCtx, env: &Env) {
        self.0.inner.init(ctx, env)
    }

    fn paint(&self, ctx: &mut PaintCtx, cell: &CellCtx, data: &T, env: &Env) {
        let inner = &self.0.inner;
        let inner_data = (self.0.wrapper)(data);
        inner.paint(ctx, cell, &inner_data, env);
    }
}

impl<T, U, F, DC> DataCompare<T> for FuncWrapped<T, U, F, DC>
where
    T: Data,
    U: Data,
    F: Fn(&T) -> U,
    DC: DataCompare<U>,
{
    fn compare(&self, a: &T, b: &T) -> Ordering {
        let a = (self.0.wrapper)(a);
        let b = (self.0.wrapper)(b);
        self.0.inner.compare(&a, &b)
    }
}

#[derive(Clone)]
pub struct TextCell {
    text_color: KeyOrValue<Color>,
    font_name: KeyOrValue<ArcStr>,
    font_size: KeyOrValue<f64>,
    cached_font: Option<FontFamily>,
}

impl TextCell {
    pub fn new() -> Self {
        TextCell {
            text_color: Color::BLACK.into(),
            font_name: ArcStr::from("Gill Sans").into(),
            font_size: theme::TEXT_SIZE_NORMAL.into(),
            cached_font: None,
        }
    }

    pub fn text_color(mut self, text_color: impl Into<KeyOrValue<Color>>) -> TextCell {
        self.text_color = text_color.into();
        self
    }

    pub fn font_name(mut self, font_name: impl Into<KeyOrValue<ArcStr>>) -> TextCell {
        self.font_name = font_name.into();
        self
    }

    pub fn font_size(mut self, font_size: impl Into<KeyOrValue<f64>>) -> TextCell {
        self.font_size = font_size.into();
        self
    }

    fn resolve_font(&self, ctx: &mut PaintCtx, env: &Env) -> FontFamily {
        let font: FontFamily = ctx
            .text()
            .font_family(&self.font_name.resolve(env))
            .unwrap(); // TODO errors / fallback
        font
    }

    fn paint_impl(&self, ctx: &mut PaintCtx, data: &str, env: &Env, font: &FontFamily) {
        // TODO: error handling
        // TODO: wrapping (multi line)

        if let Ok(layout) = ctx
            .text()
            .new_text_layout(data.to_string())
            .font(font.clone(), self.font_size.resolve(env))
            .text_color(self.text_color.resolve(env))
            .build()
        {
            ctx.draw_text(&layout, (0.0, 0.0));
        }
    }
}

impl Default for TextCell {
    fn default() -> Self {
        TextCell::new()
    }
}

impl CellRender<String> for TextCell {
    fn init(&mut self, ctx: &mut PaintCtx, env: &Env) {
        if self.cached_font.is_none() {
            let font = self.resolve_font(ctx, env);
            self.cached_font = Some(font);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx, _cell: &CellCtx, data: &String, env: &Env) {
        if let Some(font) = &self.cached_font {
            self.paint_impl(ctx, data, env, font);
        } else {
            log::warn!("Font not cached, are you missing a call to init");
            let font = self.resolve_font(ctx, env);
            ctx.stroke(
                Line::new((0., 0.), (100., 100.)),
                &Color::rgb8(0xff, 0, 0),
                2.,
            );
            self.paint_impl(ctx, data, env, &font);
        }
    }
}

impl EditorFactory<String> for TextCell {
    fn make_editor(&mut self, _ctx: &CellCtx) -> Option<Box<dyn Widget<String>>> {
        Some(Box::new(TextBox::new().expand_height()))
    }
}

pub(crate) struct HeaderCell<T, I: CellRender<T>> {
    inner: I,
    phantom_t: PhantomData<T>,
}

impl<T, I: CellRender<T>> HeaderCell<T, I> {
    pub fn new(inner: I) -> Self {
        HeaderCell {
            inner,
            phantom_t: Default::default(),
        }
    }
}

fn make_arrow(top_point: &Point, up: bool, height: f64, head_rad: f64) -> Vec<PathEl> {
    let start_y = top_point.y;
    let tip_y = start_y + height;

    let (start_y, tip_y, mult) = if up {
        (tip_y, start_y, -1.)
    } else {
        (start_y, tip_y, 1.0)
    };
    let head_start_y = tip_y - (head_rad * mult);

    let mid_x = top_point.x;

    let arrow = vec![
        PathEl::MoveTo((mid_x, start_y).into()),
        PathEl::LineTo((mid_x, tip_y).into()),
        PathEl::LineTo((mid_x - head_rad, head_start_y).into()),
        PathEl::MoveTo((mid_x, tip_y).into()),
        PathEl::LineTo((mid_x + head_rad, head_start_y).into()),
    ];
    arrow
}

impl<T, I: CellRender<T>> CellRender<T> for HeaderCell<T, I> {
    fn init(&mut self, ctx: &mut PaintCtx, env: &Env) {
        self.inner.init(ctx, env);
    }

    fn paint(&self, ctx: &mut PaintCtx, cell: &CellCtx, data: &T, env: &Env) {
        match cell {
            CellCtx::Header(_, _, Some(ss)) => {
                // TODO The size should be on the CellCtx, should not be using region
                let rect = ctx
                    .region()
                    .bounding_box()
                    .with_origin(Point::ORIGIN)
                    .inset(-3.);
                let rad = rect.height() * 0.25;
                let up = ss.direction == Ascending;

                let arrow = make_arrow(
                    &Point::new(rect.max_x() - rad, rect.min_y()),
                    up,
                    rect.height(),
                    rad,
                );
                ctx.render_ctx.stroke(&arrow[..], &Color::WHITE, 1.0);
                let rect1 = ctx.region().bounding_box();
                let rect1 = rect1
                    .with_origin(Point::ORIGIN)
                    .with_size((rect1.width() - (rad + 3.) * 2., rect1.height()));
                ctx.clip(rect1);
                self.inner.paint(ctx, cell, data, env);
            }
            _ => {
                self.inner.paint(ctx, cell, data, env);
            }
        }
    }
}

impl DataCompare<String> for TextCell {
    fn compare(&self, a: &String, b: &String) -> Ordering {
        a.cmp(b)
    }
}

pub struct TableColumn<T: Data, CD: CellDelegate<T>> {
    pub(crate) header: String,
    cell_delegate: CD,
    pub(crate) width: TableColumnWidth,
    pub(crate) sort_order: Option<usize>,
    pub(crate) sort_fixed: bool,
    pub(crate) sort_dir: Option<SortDirection>,
    phantom_: PhantomData<T>,
}

impl<T: Data, CD: CellDelegate<T>> Debug for TableColumn<T, CD> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableColumn")
            .field("header", &self.header)
            .finish()
    }
}

pub struct TableColumnWidth {
    initial: Option<KeyOrValue<f64>>,
    min: Option<KeyOrValue<f64>>,
    max: Option<KeyOrValue<f64>>,
}

impl Default for TableColumnWidth {
    fn default() -> Self {
        TableColumnWidth {
            initial: Some(50.0.into()), // Could be in a 'theme' I guess.
            min: Some(20.0.into()),
            max: None,
        }
    }
}

impl From<f64> for TableColumnWidth {
    fn from(num: f64) -> Self {
        let mut tc = TableColumnWidth::default();
        tc.initial = Some(num.into());
        tc
    }
}

impl<T1, T2, T3> From<(T1, T2, T3)> for TableColumnWidth
where
    T1: Into<KeyOrValue<f64>>,
    T2: Into<KeyOrValue<f64>>,
    T3: Into<KeyOrValue<f64>>,
{
    fn from((initial, min, max): (T1, T2, T3)) -> Self {
        TableColumnWidth {
            initial: Some(initial.into()),
            min: Some(min.into()),
            max: Some(max.into()),
        }
    }
}

pub fn column<T: Data, CD: CellDelegate<T> + 'static>(
    header: impl Into<String>,
    cell_delegate: CD,
) -> TableColumn<T, Box<dyn CellDelegate<T>>> {
    TableColumn::new(header, Box::new(cell_delegate))
}

impl<T: Data, CD: CellDelegate<T>> TableColumn<T, CD> {
    pub fn new(header: impl Into<String>, cell_delegate: CD) -> Self {
        TableColumn {
            header: header.into(),
            cell_delegate,
            sort_order: Default::default(),
            sort_fixed: false,
            sort_dir: None,
            width: Default::default(),
            phantom_: PhantomData,
        }
    }

    pub fn width<W: Into<TableColumnWidth>>(mut self, width: W) -> Self {
        self.width = width.into();
        self
    }

    pub fn sort<S: Into<SortDirection>>(mut self, sort: S) -> Self {
        self.sort_dir = Some(sort.into());
        self
    }

    pub fn fix_sort(mut self) -> Self {
        self.sort_fixed = true;
        self
    }
}

impl<T: Data, CR: CellDelegate<T>> CellRender<T> for TableColumn<T, CR> {
    fn init(&mut self, ctx: &mut PaintCtx, env: &Env) {
        self.cell_delegate.init(ctx, env)
    }

    fn paint(&self, ctx: &mut PaintCtx, cell: &CellCtx, data: &T, env: &Env) {
        self.cell_delegate.paint(ctx, cell, data, env)
    }
}

impl<T: Data, CR: CellDelegate<T>> DataCompare<T> for TableColumn<T, CR> {
    fn compare(&self, a: &T, b: &T) -> Ordering {
        self.cell_delegate.compare(a, b)
    }
}

impl<T: Data, CR: CellDelegate<T>> EditorFactory<T> for TableColumn<T, CR> {
    fn make_editor(&mut self, ctx: &CellCtx) -> Option<Box<dyn Widget<T>>> {
        self.cell_delegate.make_editor(ctx)
    }
}

pub struct ProvidedColumns<TableData: IndexedData, ColumnType: CellDelegate<TableData::Item>>
where
    TableData::Item: Data,
{
    cols: Vec<TableColumn<TableData::Item, ColumnType>>,
    phantom_td: PhantomData<TableData>,
}

impl<TableData: IndexedData, ColumnType: CellDelegate<TableData::Item>>
    ProvidedColumns<TableData, ColumnType>
where
    TableData::Item: Data,
{
    pub fn new(cols: Vec<TableColumn<TableData::Item, ColumnType>>) -> Self {
        ProvidedColumns {
            cols,
            phantom_td: Default::default(),
        }
    }
}

impl<TableData: IndexedData<Idx = LogIdx>, ColumnType: CellDelegate<TableData::Item>>
    Remapper<TableData> for ProvidedColumns<TableData, ColumnType>
where
    TableData::Item: Data,
{
    fn sort_fixed(&self, idx: usize) -> bool {
        self.cols.get(idx).map(|c| c.sort_fixed).unwrap_or(false)
    }

    fn initial_spec(&self) -> RemapSpec {
        let mut spec = RemapSpec::default();

        // Put the columns in sort order
        let mut in_order: Vec<(usize, &TableColumn<TableData::Item, ColumnType>)> =
            self.cols.iter().enumerate().collect();
        in_order.sort_by(|(_, a), (_, b)| match (a.sort_order, b.sort_order) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), _) => Ordering::Greater,
            (_, Some(_)) => Ordering::Less,
            _ => Ordering::Equal,
        });

        // Then add the ones which have a
        for (idx, dir) in in_order
            .into_iter()
            .filter_map(|(idx, c)| c.sort_dir.as_ref().map(|c| (idx, c)))
        {
            spec.add_sort(SortSpec::new(idx, *dir))
        }
        spec
    }

    fn remap_items(&self, table_data: &TableData, remap_spec: &RemapSpec) -> Remap {
        if remap_spec.is_empty() {
            Remap::new() // Todo: preserve moves
        } else {
            //Todo: Filter
            let mut idxs: Vector<LogIdx> = (0usize..table_data.idx_len()).map(LogIdx).collect(); //TODO Give up if too big?
            idxs.sort_by(|a, b| {
                table_data
                    .with(*a, |a| {
                        table_data
                            .with(*b, |b| {
                                for SortSpec { idx, direction } in &remap_spec.sort_by {
                                    let col = self.cols.get(*idx).unwrap();
                                    let ord = col.compare(a, b);
                                    if ord != Ordering::Equal {
                                        return direction.apply(ord);
                                    }
                                }
                                Ordering::Equal
                            })
                            .unwrap()
                    })
                    .unwrap()
            });
            Remap::Selected(RemapDetails::Full(idxs))
        }
    }
}

impl<TableData: IndexedData<Idx = LogIdx>, ColumnType: CellDelegate<TableData::Item>>
    CellRender<TableData::Item> for ProvidedColumns<TableData, ColumnType>
where
    TableData::Item: Data,
{
    fn init(&mut self, ctx: &mut PaintCtx, env: &Env) {
        self.cols.init(ctx, env)
    }

    fn paint(&self, ctx: &mut PaintCtx, cell: &CellCtx, data: &TableData::Item, env: &Env) {
        self.cols.paint(ctx, cell, data, env);
    }
}

impl<TableData: IndexedData<Idx = LogIdx>, ColumnType: CellDelegate<TableData::Item>>
    EditorFactory<TableData::Item> for ProvidedColumns<TableData, ColumnType>
where
    TableData::Item: Data,
{
    fn make_editor(
        &mut self,
        ctx: &CellCtx,
    ) -> Option<Box<dyn Widget<<TableData as IndexedItems>::Item>>> {
        self.cols.make_editor(ctx)
    }
}

impl<TableData: IndexedData<Idx = LogIdx>, ColumnType: CellDelegate<TableData::Item>>
    CellsDelegate<TableData> for ProvidedColumns<TableData, ColumnType>
where
    TableData::Item: Data,
{
    fn number_of_columns_in_data(&self, _data: &TableData) -> usize {
        self.cols.len()
    }
}
