use crate::app::{App, DialogKind};
use crate::theme;
use eframe::egui::{self, Align, Layout, Ui};
use lockin_core::PomoAction;

pub fn fmt_hm(minutes: i64) -> String {
    if minutes == 0 {
        return "0m".into();
    }
    let h = minutes / 60;
    let m = minutes % 60;
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

pub fn fmt_clock(ms: i64) -> String {
    let s = (ms / 1000).max(0);
    format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

#[derive(Default)]
pub struct SettingsActions {
    close: bool,
    set_category: Option<Option<i64>>,
    set_subject: Option<Option<i64>>,
    add_category: bool,
    rename_category: bool,
    delete_category: bool,
    save_cfg: bool,
}

impl App {
    pub fn view_timer(&mut self, ui: &mut Ui) {
        let remaining = self.remaining_ms();
        let running = self.pomo.running;
        let ended = self.pomo.ended;
        let phase = if ended {
            format!("{} done", self.pomo.phase)
        } else {
            self.pomo.phase.clone()
        };
        let blink = ui.input(|i| i.time * 3.2).sin() > -0.3;
        let clock_color = if ended {
            if blink { theme::RED } else { theme::DIM }
        } else if running {
            theme::BRIGHT
        } else {
            theme::GOLD
        };

        // keep the timer block vertically centered in the window
        let content_h = 180.0;
        let screen = ui.ctx().screen_rect();
        let top = ((screen.height() - content_h) / 2.0 - ui.cursor().top()).max(0.0);
        ui.add_space(top);
        ui.vertical_centered(|ui| {
            ui.label(theme::b(phase.to_lowercase(), 12.0).color(theme::MUTED));
            ui.add_space(4.0);
            theme::centered_big(ui, &fmt_clock(remaining), 44.0, clock_color);
            ui.add_space(14.0);
            let toggle = if running {
                "pause"
            } else if ended || remaining <= 0 {
                "start next"
            } else {
                "start"
            };
            let total = theme::text_width(ui, toggle, theme::regular_font(13.0))
                + theme::text_width(ui, "reset", theme::regular_font(13.0))
                + theme::text_width(ui, "skip", theme::regular_font(13.0))
                + 32.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                if theme::link(ui, toggle, 13.0, theme::RED).clicked() {
                    let action = if running {
                        PomoAction::Pause
                    } else {
                        PomoAction::Start
                    };
                    let _ = self.db.pomo_action(action);
                    self.reload_timer();
                    self.reload_tasks();
                }
                if theme::link(ui, "reset", 13.0, theme::RED).clicked() {
                    let _ = self.db.pomo_action(PomoAction::Reset);
                    self.reload_timer();
                }
                if theme::link(ui, "skip", 13.0, theme::RED).clicked() {
                    let _ = self.db.pomo_action(PomoAction::Skip);
                    self.reload_timer();
                }
            });
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let total = theme::text_width(ui, "today ", theme::regular_font(13.0))
                    + 4.0
                    + theme::text_width(
                        ui,
                        &fmt_hm(self.pomo.today_focused_min),
                        theme::bold_font(13.0),
                    );
                ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                ui.label(theme::t("today ", 13.0).color(theme::MUTED));
                ui.label(theme::b(fmt_hm(self.pomo.today_focused_min), 13.0).color(theme::BRIGHT));
            });
            ui.add_space(14.0);
            if theme::link(ui, "timer settings", 13.0, theme::RED).clicked() {
                self.settings_open = !self.settings_open;
                if self.settings_open {
                    self.cfg_focus = self.pomo.focus_min.to_string();
                    self.cfg_break = self.pomo.break_min.to_string();
                    self.cfg_auto = self.pomo.auto_next;
                }
            }
        });
    }

    pub fn settings_popup(&mut self, ctx: &egui::Context) {
        let mut act = SettingsActions::default();
        let screen_w = ctx.screen_rect().width();
        let wide = screen_w >= 500.0;
        let width = (screen_w - 64.0).clamp(300.0, 540.0);

        egui::Area::new(egui::Id::new("settings-popup"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fade_in(false)
            .show(ctx, |ui| {
                theme::textured_panel(
                    ui,
                    self.ceramic.id(),
                    Some(width),
                    egui::Margin::symmetric(16, 12),
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.label(theme::b("timer settings", 13.0).color(theme::BRIGHT));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if popup_close(ui) {
                                    act.close = true;
                                }
                            });
                        });
                        ui.add_space(10.0);
                        let max_h = (ctx.screen_rect().height() - 96.0).max(120.0);
                        egui::ScrollArea::vertical()
                            .max_height(max_h)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                if wide {
                                    ui.columns(2, |cols| {
                                        self.settings_timer_section(&mut cols[0], &mut act);
                                        self.settings_context_section(&mut cols[1], &mut act);
                                    });
                                } else {
                                    self.settings_timer_section(ui, &mut act);
                                    ui.add_space(14.0);
                                    self.settings_context_section(ui, &mut act);
                                }
                            });
                    },
                );
            });

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            act.close = true;
        }
        if let Some(cat) = act.set_category {
            let _ = self.db.pomo_action(PomoAction::SetCategory(cat));
            self.reload_timer();
        }
        if let Some(sub) = act.set_subject {
            let _ = self.db.pomo_action(PomoAction::SetSubject(sub));
            self.reload_timer();
        }
        if act.add_category {
            let name = self.category_new.trim().to_string();
            if !name.is_empty() {
                if let Ok(id) = self.db.add_category(&name) {
                    self.category_new.clear();
                    self.reload_categories();
                    let _ = self.db.pomo_action(PomoAction::SetCategory(Some(id)));
                    self.reload_timer();
                }
            }
        }
        if act.save_cfg {
            let focus: i64 = self.cfg_focus.trim().parse().unwrap_or(-1);
            let brk: i64 = self.cfg_break.trim().parse().unwrap_or(-1);
            if !(1..=600).contains(&focus) || !(1..=600).contains(&brk) {
                self.cfg_msg = Some((
                    "focus and break must be 1..600".into(),
                    false,
                    std::time::Instant::now(),
                ));
            } else {
                let _ = self.db.pomo_action(PomoAction::Config {
                    focus_min: focus,
                    break_min: brk,
                    auto_next: self.cfg_auto,
                });
                self.reload_timer();
                self.cfg_msg = Some(("saved ✓".into(), true, std::time::Instant::now()));
            }
        }
        if act.rename_category {
            if let Some((id, name)) = self.pomo.category.as_ref().map(|c| (c.id, c.name.clone())) {
                self.open_prompt(DialogKind::RenameCategory(id), "rename category", "name", &name);
            }
        }
        if act.delete_category {
            if let Some((id, name)) = self.pomo.category.as_ref().map(|c| (c.id, c.name.clone())) {
                let label = format!("delete \"{name}\"? its focus periods become Unallocated.");
                self.open_prompt(
                    DialogKind::ConfirmDeleteCategory(id, name),
                    "delete category",
                    &label,
                    "",
                );
            }
        }
        if act.close {
            self.settings_open = false;
        }
    }

    fn settings_timer_section(&mut self, ui: &mut Ui, act: &mut SettingsActions) {
        ui.label(theme::b("timer", 13.0).color(theme::GOLD));
        ui.horizontal(|ui| {
            ui.label(theme::i("focus", 12.0).color(theme::MUTED));
            ui.add_sized(
                [48.0, 24.0],
                egui::TextEdit::singleline(&mut self.cfg_focus).font(theme::regular_font(13.0)),
            );
            ui.label(theme::i("break", 12.0).color(theme::MUTED));
            ui.add_sized(
                [48.0, 24.0],
                egui::TextEdit::singleline(&mut self.cfg_break).font(theme::regular_font(13.0)),
            );
        });
        ui.add_space(4.0);
        ui.checkbox(&mut self.cfg_auto, "auto-start next phase");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if theme::link(ui, "save", 13.0, theme::RED).clicked() {
                act.save_cfg = true;
            }
            if let Some((msg, ok, at)) = &self.cfg_msg {
                if at.elapsed().as_secs() < 3 {
                    ui.label(theme::tc(
                        msg,
                        12.0,
                        if *ok { theme::MUTED } else { theme::RED_HOT },
                    ));
                }
            }
        });
    }

    fn settings_context_section(&mut self, ui: &mut Ui, act: &mut SettingsActions) {
        let cats: Vec<(i64, String)> = self
            .categories
            .iter()
            .map(|c| (c.id, c.name.clone()))
            .collect();
        let subjects = self.subjects.clone();
        let cur_cat = self.pomo.category.as_ref().map(|c| c.id);
        let cur_subj = self.pomo.subject.as_ref().map(|s| s.id);

        ui.label(theme::b("context", 13.0).color(theme::GOLD));
        ui.label(theme::i("category", 12.0).color(theme::MUTED));
        let selected = cats
            .iter()
            .find(|(id, _)| Some(*id) == cur_cat)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| "Unallocated".into());
        let combo_w = ui.available_width().min(220.0);
        egui::ComboBox::from_id_salt("cat-select")
            .width(combo_w)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui.selectable_label(cur_cat.is_none(), "Unallocated").clicked() {
                    act.set_category = Some(None);
                }
                for (id, name) in &cats {
                    if ui.selectable_label(Some(*id) == cur_cat, name).clicked() {
                        act.set_category = Some(Some(*id));
                    }
                }
            });
        ui.add(
            egui::TextEdit::singleline(&mut self.category_new)
                .hint_text("new category...")
                .desired_width(f32::INFINITY)
                .font(theme::regular_font(13.0)),
        );
        ui.horizontal(|ui| {
            if theme::link(ui, "+ add", 12.0, theme::RED).clicked() {
                act.add_category = true;
            }
            if theme::link(ui, "rename", 12.0, theme::RED).clicked() {
                act.rename_category = true;
            }
            if theme::link(ui, "delete", 12.0, theme::RED).clicked() {
                act.delete_category = true;
            }
        });
        ui.add_space(6.0);
        ui.label(theme::i("subject", 12.0).color(theme::MUTED));
        let selected = self
            .pomo
            .subject
            .as_ref()
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "none".into());
        let combo_w = ui.available_width().min(220.0);
        egui::ComboBox::from_id_salt("subj-select")
            .width(combo_w)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui.selectable_label(cur_subj.is_none(), "none").clicked() {
                    act.set_subject = Some(None);
                }
                for (id, name, group) in &subjects {
                    if ui
                        .selectable_label(Some(*id) == cur_subj, format!("{group} · {name}"))
                        .clicked()
                    {
                        act.set_subject = Some(Some(*id));
                    }
                }
            });
    }

    pub fn view_tracker(&mut self, ui: &mut Ui) {
        if self.tracker.is_none() {
            self.reload_tracker();
        }
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                let (rect, response) = ui.allocate_exact_size(egui::Vec2::new(16.0, 20.0), egui::Sense::click());
                let color = if response.hovered() { theme::RED_HOT } else { theme::RED };
                theme::paint_plus(ui.painter(), rect.center(), 11.0, color);
                if response.clicked() {
                    self.open_prompt(crate::app::DialogKind::AddGroup, "add group", "group name (e.g. stream processing)", "");
                }
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
        });
        ui.add_space(2.0);

        let groups = self
            .tracker
            .as_ref()
            .map(|t| t.groups.clone())
            .unwrap_or_default();
        if groups.is_empty() {
            theme::empty_note(ui, "no groups yet, hit + to add one (e.g. stream processing)", 0.0);
            return;
        }
        for group in groups {
            self.tracker_group(ui, &group);
        }

        if ui.input(|i| i.pointer.any_released()) {
            if let (Some(from), Some(to)) = (self.drag_group, self.drag_group_target) {
                if from != to {
                    let mut order: Vec<i64> = self
                        .tracker
                        .as_ref()
                        .map(|t| t.groups.iter().map(|g| g.id).collect())
                        .unwrap_or_default();
                    if let (Some(f), Some(t)) = (
                        order.iter().position(|id| *id == from),
                        order.iter().position(|id| *id == to),
                    ) {
                        let id = order.remove(f);
                        order.insert(t, id);
                        let _ = self.db.reorder_groups(&order);
                        self.reload_tracker();
                    }
                }
            }
            if let (Some((gid, from)), Some(to)) = (self.drag_subject, self.drag_subject_target) {
                if from != to {
                    let mut order: Vec<i64> = self
                        .tracker
                        .as_ref()
                        .and_then(|t| t.groups.iter().find(|g| g.id == gid))
                        .map(|g| g.subjects.iter().map(|s| s.id).collect())
                        .unwrap_or_default();
                    if let (Some(f), Some(t)) = (
                        order.iter().position(|id| *id == from),
                        order.iter().position(|id| *id == to),
                    ) {
                        let id = order.remove(f);
                        order.insert(t, id);
                        let _ = self.db.reorder_subjects(&order);
                        self.reload_tracker();
                    }
                }
            }
            self.drag_group = None;
            self.drag_group_target = None;
            self.drag_subject = None;
            self.drag_subject_target = None;
        }
    }

    fn tracker_group(&mut self, ui: &mut Ui, group: &lockin_core::TrackerGroup) {
        let width = ui.available_width();
        let (row, response) = ui.allocate_exact_size(egui::Vec2::new(width, 22.0), egui::Sense::click_and_drag());
        let hovered = response.hovered();
        if response.drag_started() {
            self.drag_group = Some(group.id);
        }
        if self.drag_group.is_some() && hovered {
            self.drag_group_target = Some(group.id);
        }
        let dragging = self.drag_group == Some(group.id);
        let drop_target = self.drag_group_target == Some(group.id)
            && self.drag_group.is_some()
            && self.drag_group != Some(group.id);
        if dragging || drop_target {
            ui.painter().rect_filled(
                row,
                0.0,
                egui::Color32::from_rgba_unmultiplied(215, 160, 75, 12),
            );
        }
        if drop_target {
            ui.painter().line_segment(
                [row.left_top(), row.right_top()],
                egui::Stroke::new(2.0_f32, theme::GOLD),
            );
        }
        let collapsed = !self.group_expanded.contains(&group.id);
        let caret_rect = egui::Rect::from_min_size(row.min, egui::Vec2::new(16.0, row.height()));
        let caret_resp = ui.interact(caret_rect, ui.id().with(("gcaret", group.id)), egui::Sense::click());
        theme::paint_caret(
            ui.painter(),
            caret_rect.center(),
            7.0,
            theme::MUTED,
            if collapsed { theme::Caret::Right } else { theme::Caret::Down },
        );
        if caret_resp.clicked() {
            if collapsed {
                self.group_expanded.insert(group.id);
            } else {
                self.group_expanded.remove(&group.id);
            }
            self.save_tracker_collapse();
        }
        if caret_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let name_galley = ui.painter().layout_no_wrap(
            group.name.clone(),
            theme::bold_font(14.0),
            theme::GOLD,
        );
        let name_rect = egui::Rect::from_center_size(
            egui::Pos2::new(row.left() + 16.0 + name_galley.size().x / 2.0, row.center().y),
            name_galley.size() + egui::Vec2::new(2.0, 0.0),
        );
        ui.painter().galley(
            egui::Pos2::new(name_rect.left(), row.center().y - name_galley.size().y / 2.0),
            name_galley.clone(),
            theme::GOLD,
        );
        let name_resp = ui.interact(
            name_rect,
            ui.id().with(("gname", group.id)),
            egui::Sense::click_and_drag(),
        );
        if name_resp.drag_started() {
            self.drag_group = Some(group.id);
        }
        if self.drag_group.is_some() && name_resp.hovered() {
            self.drag_group_target = Some(group.id);
        }
        if name_resp.clicked() {
            self.open_prompt(
                crate::app::DialogKind::RenameGroup(group.id),
                "rename group",
                "name",
                &group.name,
            );
        }
        if name_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }

        let pct = group
            .pct
            .map(|p| format!("{p}%"))
            .unwrap_or_else(|| "–".into());
        let meta = format!("{pct} · {}/{} done", group.done, group.count);
        let meta_galley = ui.painter().layout_no_wrap(
            meta,
            theme::italic_font(11.0),
            theme::MUTED,
        );
        ui.painter().galley(
            egui::Pos2::new(
                name_rect.right() + 8.0,
                row.center().y - meta_galley.size().y / 2.0,
            ),
            meta_galley,
            theme::MUTED,
        );

        let trash_rect = egui::Rect::from_center_size(
            egui::Pos2::new(row.right() - 10.0, row.center().y),
            egui::Vec2::new(11.0, 13.0),
        );
        let trash_resp = ui.interact(trash_rect, ui.id().with(("gtrash", group.id)), egui::Sense::click());
        if hovered || trash_resp.hovered() {
            theme::paint_trash(
                ui.painter(),
                trash_rect,
                if trash_resp.hovered() { theme::RED_HOT } else { theme::MUTED },
            );
        }
        if trash_resp.clicked() {
            self.open_prompt(
                crate::app::DialogKind::ConfirmDeleteGroup(
                    group.id,
                    group.name.clone(),
                    group.subjects.len(),
                ),
                "delete group",
                &format!(
                    "delete \"{}\" and its {} subject{}?",
                    group.name,
                    group.subjects.len(),
                    if group.subjects.len() == 1 { "" } else { "s" }
                ),
                "",
            );
        }

        if collapsed {
            ui.add_space(14.0);
            return;
        }

        ui.spacing_mut().item_spacing.y = 0.0;
        let open: Vec<_> = group
            .subjects
            .iter()
            .filter(|s| !matches!(s.state.as_str(), "done" | "dropped" | "paused"))
            .cloned()
            .collect();
        let hold: Vec<_> = group
            .subjects
            .iter()
            .filter(|s| s.state == "paused")
            .cloned()
            .collect();
        let done: Vec<_> = group
            .subjects
            .iter()
            .filter(|s| s.state == "done")
            .cloned()
            .collect();
        let dropped: Vec<_> = group
            .subjects
            .iter()
            .filter(|s| s.state == "dropped")
            .cloned()
            .collect();

        for s in &open {
            self.tracker_subject_row(ui, group, s);
        }
        if !hold.is_empty() {
            ui.add_space(2.0);
            let mut clicked = false;
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                clicked = theme::caret_link(
                    ui,
                    &format!("on hold ({})", hold.len()),
                    if self.hold_collapsed { theme::Caret::Right } else { theme::Caret::Down },
                )
                .clicked();
            });
            if clicked {
                self.hold_collapsed = !self.hold_collapsed;
                self.save_tracker_collapse();
            }
            if !self.hold_collapsed {
                for s in &hold {
                    self.tracker_subject_row(ui, group, s);
                }
            }
        }
        if !done.is_empty() {
            ui.add_space(2.0);
            let mut clicked = false;
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                clicked = theme::caret_link(
                    ui,
                    &format!("done ({})", done.len()),
                    if self.done_collapsed_trk { theme::Caret::Right } else { theme::Caret::Down },
                )
                .clicked();
            });
            if clicked {
                self.done_collapsed_trk = !self.done_collapsed_trk;
                self.save_tracker_collapse();
            }
            if !self.done_collapsed_trk {
                for s in &done {
                    self.tracker_subject_row(ui, group, s);
                }
            }
        }
        if !dropped.is_empty() {
            ui.add_space(2.0);
            let mut clicked = false;
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                clicked = theme::caret_link(
                    ui,
                    &format!("dropped ({})", dropped.len()),
                    if self.drop_collapsed { theme::Caret::Right } else { theme::Caret::Down },
                )
                .clicked();
            });
            if clicked {
                self.drop_collapsed = !self.drop_collapsed;
                self.save_tracker_collapse();
            }
            if !self.drop_collapsed {
                for s in &dropped {
                    self.tracker_subject_row(ui, group, s);
                }
            }
        }

        ui.add_space(3.0);
        if self.add_subject_group == Some(group.id) {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let width = ui.available_width();
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.add_subject_text)
                        .hint_text("add subject...")
                        .desired_width(width - 60.0)
                        .font(theme::regular_font(13.0)),
                );
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.commit_add_subject(group.id);
                }
                if edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.add_subject_group = None;
                }
                edit.request_focus();
                if theme::bevel(ui, "add", true).clicked() {
                    self.commit_add_subject(group.id);
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let (rect, response) = ui.allocate_exact_size(egui::Vec2::new(14.0, 18.0), egui::Sense::click());
                let color = if response.hovered() { theme::RED_HOT } else { theme::RED };
                theme::paint_plus(ui.painter(), rect.center(), 11.0, color);
                if response.clicked() {
                    self.add_subject_group = Some(group.id);
                    self.add_subject_text.clear();
                }
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
        }
        ui.add_space(18.0);
    }

    fn commit_add_subject(&mut self, group_id: i64) {
        let name = self.add_subject_text.trim().to_string();
        if name.is_empty() {
            self.add_subject_group = None;
            return;
        }
        if self.db.add_subject(group_id, &name, "other", "", "", None).is_ok() {
            self.add_subject_text.clear();
            self.add_subject_group = None;
            self.reload_tracker();
        }
    }

    fn tracker_subject_row(
        &mut self,
        ui: &mut Ui,
        group: &lockin_core::TrackerGroup,
        subject: &lockin_core::TrackerSubject,
    ) {
        let width = ui.available_width();
        let indent = 7.0;
        let height = 20.0;
        let (row, response) = ui.allocate_exact_size(egui::Vec2::new(width, height), egui::Sense::click_and_drag());
        let hovered = response.hovered();
        if response.drag_started() {
            self.drag_subject = Some((group.id, subject.id));
        }
        if let Some((gid, _)) = self.drag_subject {
            if gid == group.id && hovered {
                self.drag_subject_target = Some(subject.id);
            }
        }
        let dragging = self.drag_subject == Some((group.id, subject.id));
        let drop_target = self.drag_subject_target == Some(subject.id)
            && self.drag_subject.map(|(gid, id)| (gid, id)) != Some((group.id, subject.id))
            && self.drag_subject.map(|(gid, _)| gid) == Some(group.id);
        if dragging || drop_target {
            ui.painter().rect_filled(
                row,
                0.0,
                egui::Color32::from_rgba_unmultiplied(215, 160, 75, 12),
            );
        }
        if drop_target {
            ui.painter().line_segment(
                [row.left_top(), row.right_top()],
                egui::Stroke::new(2.0_f32, theme::GOLD),
            );
        }
        if hovered {
            ui.painter().rect_filled(
                row,
                0.0,
                egui::Color32::from_rgba_unmultiplied(215, 160, 90, 7),
            );
        }

        let star_rect = egui::Rect::from_center_size(
            egui::Pos2::new(row.left() + indent + 7.0, row.center().y),
            egui::Vec2::new(14.0, 20.0),
        );
        let star_resp = ui.interact(star_rect, ui.id().with(("star", subject.id)), egui::Sense::click());
        let star_color = if subject.is_next {
            theme::BRIGHT
        } else if star_resp.hovered() {
            theme::BRIGHT
        } else {
            theme::DIM
        };
        theme::paint_star(ui.painter(), star_rect.center(), 5.0, subject.is_next, star_color);
        if star_resp.clicked() {
            let _ = self.db.tracker_set_next(subject.id, !subject.is_next);
            self.reload_tracker();
        }
        if star_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let glyph_rect = egui::Rect::from_min_size(
            egui::Pos2::new(star_rect.right() + 8.0, row.top()),
            egui::Vec2::new(12.0, height),
        );
        let glyph_resp = ui.interact(glyph_rect, ui.id().with(("sglyph", subject.id)), egui::Sense::click());
        if subject.state == "doing" {
            ui.painter().image(
                self.arrow.id(),
                egui::Rect::from_min_size(
                    egui::Pos2::new(glyph_rect.left(), row.center().y - 6.0),
                    egui::Vec2::new(9.0, 12.0),
                ),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            let color = match subject.state.as_str() {
                "done" | "dropped" => theme::DIM,
                "paused" => theme::MUTED,
                _ => theme::GOLD,
            };
            let galley = ui.painter().layout_no_wrap("·".into(), theme::regular_font(15.0), color);
            ui.painter().galley(
                egui::Pos2::new(glyph_rect.center().x - galley.size().x / 2.0, row.center().y - galley.size().y / 2.0),
                galley,
                color,
            );
        }
        if glyph_resp.clicked() {
            let next = match subject.state.as_str() {
                "todo" => "doing",
                "doing" => "done",
                "done" => "todo",
                "paused" => "doing",
                "dropped" => "todo",
                _ => "todo",
            };
            let _ = self.db.subject_state(subject.id, next);
            self.reload_tracker();
        }

        let text_x = glyph_rect.right() + 8.0;
        let (color, family) = match subject.state.as_str() {
            "doing" => (theme::BRIGHT, theme::bold_family()),
            "paused" => (theme::MUTED, theme::italic_family()),
            "done" => (theme::DIM, theme::regular_family()),
            "dropped" => (theme::FAINT, theme::regular_family()),
            _ => (theme::GOLD, theme::regular_family()),
        };
        let galley = ui.painter().layout_no_wrap(subject.name.clone(), egui::FontId::new(13.0, family), color);
        let name_rect = egui::Rect::from_min_size(
            egui::Pos2::new(text_x, row.center().y - galley.size().y / 2.0),
            galley.size(),
        );
        ui.painter().galley(name_rect.min, galley.clone(), color);
        if matches!(subject.state.as_str(), "done" | "dropped") {
            theme::strike(ui.painter(), name_rect, color);
        }
        let name_resp = ui.interact(
            name_rect,
            ui.id().with(("sname", subject.id)),
            egui::Sense::click_and_drag(),
        );
        if name_resp.drag_started() {
            self.drag_subject = Some((group.id, subject.id));
        }
        if let Some((gid, _)) = self.drag_subject {
            if gid == group.id && name_resp.hovered() {
                self.drag_subject_target = Some(subject.id);
            }
        }
        if name_resp.clicked() {
            if self.tracker_editor == Some(subject.id) {
                self.tracker_editor = None;
            } else {
                self.tracker_editor = Some(subject.id);
                self.trk_name = subject.name.clone();
                self.trk_url = subject.url.clone();
                self.trk_notes = subject.notes.clone();
            }
        }
        if name_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let mut tail = name_rect.right() + 6.0;
        if !subject.url.is_empty() {
            let link_rect = egui::Rect::from_center_size(
                egui::Pos2::new(tail + 5.0, row.center().y),
                egui::Vec2::new(7.0, 7.0),
            );
            theme::paint_link_arrow(ui.painter(), link_rect, theme::RED);
            tail += 14.0;
        }

        if subject.state == "paused" {
            ui.painter().text(
                egui::Pos2::new(tail, row.center().y),
                egui::Align2::LEFT_CENTER,
                "on hold",
                theme::italic_font(11.0),
                theme::MUTED,
            );
        } else if subject.stale {
            let label = format!("! {}d", subject.days_idle.unwrap_or(0));
            let galley = ui.painter().layout_no_wrap(label.clone(), theme::italic_font(11.0), theme::RED);
            let badge_rect = egui::Rect::from_min_size(
                egui::Pos2::new(tail, row.center().y - galley.size().y / 2.0),
                galley.size(),
            );
            let badge_resp = ui.interact(badge_rect, ui.id().with(("stale", subject.id)), egui::Sense::click());
            let c = if badge_resp.hovered() { theme::RED_HOT } else { theme::RED };
            let galley = ui.painter().layout_no_wrap(label, theme::italic_font(11.0), c);
            ui.painter().galley(badge_rect.min, galley, c);
            if badge_resp.clicked() {
                self.open_prompt(
                    crate::app::DialogKind::SnoozeDays(subject.id),
                    "snooze",
                    "snooze for how many days?",
                    "7",
                );
            }
        } else if subject.snoozed {
            let until = subject
                .snooze_until
                .as_deref()
                .map(fmt_date)
                .unwrap_or_default();
            ui.painter().text(
                egui::Pos2::new(tail, row.center().y),
                egui::Align2::LEFT_CENTER,
                format!("zzz until {until}"),
                theme::italic_font(11.0),
                theme::MUTED,
            );
        }

        let kebab_rect = egui::Rect::from_center_size(
            egui::Pos2::new(row.right() - 8.0, row.center().y),
            egui::Vec2::new(16.0, height),
        );
        let kebab_resp = ui.interact(kebab_rect, ui.id().with(("kebab", subject.id)), egui::Sense::click());
        theme::paint_kebab(
            ui.painter(),
            kebab_rect.center(),
            if kebab_resp.hovered() { theme::MUTED } else { theme::FAINT },
        );
        if kebab_resp.clicked() {
            self.tracker_menu = Some((subject.id, kebab_rect.left_bottom() + egui::Vec2::new(-120.0, 4.0)));
        }

        if self.tracker_editor == Some(subject.id) {
            self.tracker_editor(ui, subject);
        }
        let _ = group;
    }

    fn tracker_editor(&mut self, ui: &mut Ui, subject: &lockin_core::TrackerSubject) {
        ui.horizontal(|ui| {
            ui.add_space(37.0);
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 20.0).max(160.0));
                ui.label(theme::i("name", 12.0).color(theme::MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut self.trk_name)
                        .desired_width(f32::INFINITY)
                        .font(theme::regular_font(13.0)),
                );
                ui.label(theme::i("resource link (url)", 12.0).color(theme::MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut self.trk_url)
                        .hint_text("https://...")
                        .desired_width(f32::INFINITY)
                        .font(theme::regular_font(13.0)),
                );
                ui.label(theme::i("notes", 12.0).color(theme::MUTED));
                ui.add(
                    egui::TextEdit::multiline(&mut self.trk_notes)
                        .desired_width(f32::INFINITY)
                        .desired_rows(2)
                        .font(theme::regular_font(13.0)),
                );
                ui.horizontal(|ui| {
                    if theme::link(ui, "save", 13.0, theme::RED).clicked() {
                        let name = self.trk_name.trim().to_string();
                        if !name.is_empty()
                            && self
                                .db
                                .set_subject_fields(
                                    subject.id,
                                    Some(&name),
                                    Some(&self.trk_url),
                                    Some(&self.trk_notes),
                                    None,
                                    None,
                                )
                                .is_ok()
                        {
                            self.reload_tracker();
                            self.toast("saved ✓");
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        if theme::link(ui, "close", 12.0, theme::MUTED).clicked() {
                            self.tracker_editor = None;
                        }
                    });
                });
                ui.add_space(4.0);
            });
        });
    }

    pub fn view_summary(&mut self, ui: &mut Ui) {
        if self.summary.is_none() {
            self.reload_summary();
        }
        let Some(summary) = self.summary.clone() else { return };
        let mut open_card: Option<lockin_core::CardKind> = None;

        Self::section_title(ui, "summary", "");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(theme::b(fmt_hm(summary.total_min), 13.0).color(theme::BRIGHT));
            ui.label(theme::i("total focused", 11.0).color(theme::MUTED));
            ui.label(theme::tc(" ·  ", 12.0, theme::FAINT));
            ui.label(theme::b(fmt_hm(summary.avg_per_focus_day), 13.0).color(theme::BRIGHT));
            ui.label(theme::i("average focused", 11.0).color(theme::MUTED));
            ui.label(theme::tc(" ·  ", 12.0, theme::FAINT));
            ui.label(theme::b(format!("{}d", summary.streak_current), 13.0).color(theme::BRIGHT));
            ui.label(theme::i("current streak", 11.0).color(theme::MUTED));
        });

        ui.add_space(20.0);
        ui.label(theme::b("last 7 days by category", 13.0).color(theme::GOLD));
        if summary.daily_categories.iter().all(|d| d.total == 0) {
            theme::empty_note(ui, "no focus in the last 7 days", 16.0);
        } else {
            for day in &summary.daily_categories {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    ui.add_space(16.0);
                    ui.label(theme::b(day.label.clone(), 13.0).color(theme::BRIGHT));
                    if day.total == 0 {
                        ui.label(theme::tc("—", 13.0, theme::GOLD));
                    } else {
                        ui.label(theme::t(fmt_hm(day.total), 13.0).color(theme::GOLD));
                    }
                    if !day.cats.is_empty() {
                        let inside = day
                            .cats
                            .iter()
                            .map(|(name, min)| format!("{name} {}", fmt_hm(*min)))
                            .collect::<Vec<_>>()
                            .join(", ");
                        ui.label(theme::i(format!("( {inside} )"), 11.0).color(theme::MUTED));
                    }
                });
            }
        }

        ui.add_space(22.0);
        ui.label(theme::b("weekly", 13.0).color(theme::GOLD));
        for week in &summary.weeks {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                ui.add_space(16.0);
                ui.label(theme::b(week.label.clone(), 13.0).color(theme::BRIGHT));
                if week.min == 0 {
                    ui.label(theme::tc("—", 13.0, theme::GOLD));
                } else {
                    ui.label(theme::t(fmt_hm(week.min), 13.0).color(theme::GOLD));
                }
            });
        }

        ui.add_space(22.0);
        ui.label(theme::b("monthly", 13.0).color(theme::GOLD));
        if summary.months.is_empty() {
            theme::empty_note(ui, "no months tracked yet", 16.0);
        }
        for month in &summary.months {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                ui.add_space(16.0);
                ui.label(theme::b(month.label.clone(), 13.0).color(theme::BRIGHT));
                ui.label(theme::t(fmt_hm(month.min), 13.0).color(theme::GOLD));
            });
        }

        ui.add_space(26.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(theme::t("export:", 12.0).color(theme::MUTED));
            if theme::link(ui, "today", 12.0, theme::RED).clicked() {
                open_card = Some(lockin_core::CardKind::Day);
            }
            ui.label(theme::tc("·", 12.0, theme::FAINT));
            if theme::link(ui, "week", 12.0, theme::RED).clicked() {
                open_card = Some(lockin_core::CardKind::Week);
            }
            ui.label(theme::tc("·", 12.0, theme::FAINT));
            if theme::link(ui, "month", 12.0, theme::RED).clicked() {
                if let Ok((y, m)) = parse_month(&self.export_month) {
                    open_card = Some(lockin_core::CardKind::Month { year: y, month: m });
                }
            }
            let months = self.months.clone();
            let shown = months
                .iter()
                .find(|m| **m == self.export_month)
                .cloned()
                .unwrap_or_else(|| self.export_month.clone());
            egui::ComboBox::from_id_salt("export-month")
                .width(130.0)
                .selected_text(pretty_month(&shown))
                .show_ui(ui, |ui| {
                    for m in &months {
                        if ui.selectable_label(*m == self.export_month, pretty_month(m)).clicked() {
                            self.export_month = m.clone();
                        }
                    }
                });
        });

        if let Some(kind) = open_card {
            self.export_card(kind);
        }
    }

    pub fn view_settings(&mut self, ui: &mut Ui) {
        if self.settings_needs_sync {
            let (w, h) = self.native_window_size(ui.ctx());
            if (400..=4000).contains(&w) && (300..=3000).contains(&h) {
                self.settings_w = w.to_string();
                self.settings_h = h.to_string();
                self.settings_zoom = format!("{:.0}", self.zoom * 100.0);
                self.settings_needs_sync = false;
            }
        }
        Self::section_title(ui, "settings", "");
        ui.add_space(12.0);

        ui.label(theme::b("window", 13.0).color(theme::GOLD));
        ui.label(theme::i("default size — resizing the window saves automatically", 11.0).color(theme::MUTED));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(theme::i("width", 12.0).color(theme::MUTED));
            ui.add_sized(
                [76.0, 24.0],
                egui::TextEdit::singleline(&mut self.settings_w).font(theme::regular_font(13.0)),
            );
            ui.add_space(8.0);
            ui.label(theme::i("height", 12.0).color(theme::MUTED));
            ui.add_sized(
                [76.0, 24.0],
                egui::TextEdit::singleline(&mut self.settings_h).font(theme::regular_font(13.0)),
            );
            if theme::link(ui, "apply", 13.0, theme::RED).clicked() {
                let w = self.settings_w.trim().parse::<f32>().unwrap_or(-1.0);
                let h = self.settings_h.trim().parse::<f32>().unwrap_or(-1.0);
                if w < 640.0 || w > 4000.0 || h < 620.0 || h > 3000.0 {
                    self.settings_msg = Some((
                        "width 640–4000, height 620–3000".into(),
                        false,
                        std::time::Instant::now(),
                    ));
                } else {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(
                        egui::Vec2::new(w, h),
                    ));
                    let (wi, hi) = (w.round() as u32, h.round() as u32);
                    self.last_window = (wi, hi);
                    self.pending_window = (wi, hi);
                    self.window_save_at = None;
                    let _ = self.db.set_setting("ui_window_w", &wi.to_string());
                    let _ = self.db.set_setting("ui_window_h", &hi.to_string());
                    self.settings_msg = Some((
                        "window size saved".into(),
                        true,
                        std::time::Instant::now(),
                    ));
                }
            }
            if theme::link(ui, "use current", 12.0, theme::RED).clicked() {
                let (w, h) = self.native_window_size(ui.ctx());
                self.settings_w = w.to_string();
                self.settings_h = h.to_string();
            }
            if theme::link(ui, "reset", 12.0, theme::RED).clicked() {
                self.settings_w = "960".into();
                self.settings_h = "780".into();
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                        960.0, 780.0,
                    )));
                self.last_window = (960, 780);
                self.pending_window = (960, 780);
                let _ = self.db.set_setting("ui_window_w", "960");
                let _ = self.db.set_setting("ui_window_h", "780");
                self.settings_msg = Some((
                    "reset to 960 × 780".into(),
                    true,
                    std::time::Instant::now(),
                ));
            }
        });
        let (cw, ch) = self.native_window_size(ui.ctx());
        ui.label(theme::i(format!("current: {cw} × {ch}"), 11.0).color(theme::DIM));

        ui.add_space(24.0);
        ui.label(theme::b("zoom", 13.0).color(theme::GOLD));
        ui.label(theme::i("applies to the whole ui · saved", 11.0).color(theme::MUTED));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_sized(
                [64.0, 24.0],
                egui::TextEdit::singleline(&mut self.settings_zoom)
                    .font(theme::regular_font(13.0)),
            );
            ui.label(theme::t("%", 12.0).color(theme::MUTED));
            if theme::link(ui, "apply", 13.0, theme::RED).clicked() {
                let z = self.settings_zoom.trim().parse::<f32>().unwrap_or(-1.0);
                if !(90.0..=200.0).contains(&z) {
                    self.settings_msg = Some((
                        "zoom 90–200%".into(),
                        false,
                        std::time::Instant::now(),
                    ));
                } else {
                    self.set_zoom(ui.ctx(), z / 100.0);
                    self.settings_msg = Some((
                        format!("zoom {:.0}% saved", z),
                        true,
                        std::time::Instant::now(),
                    ));
                }
            }
            if theme::link(ui, "reset 155%", 12.0, theme::RED).clicked() {
                self.settings_zoom = "155".into();
                self.set_zoom(ui.ctx(), 1.55);
                self.settings_msg = Some((
                    "zoom 155% saved".into(),
                    true,
                    std::time::Instant::now(),
                ));
            }
        });
        ui.label(
            theme::i("Ctrl + / Ctrl - / Ctrl 0 also zoom", 11.0).color(theme::DIM),
        );
        if let Some((msg, ok, at)) = &self.settings_msg {
            if at.elapsed().as_secs() < 3 {
                ui.add_space(10.0);
                ui.label(theme::tc(
                    msg,
                    12.0,
                    if *ok { theme::MUTED } else { theme::RED_HOT },
                ));
            }
        }
    }

    pub fn view_log(&mut self, ui: &mut Ui) {
        if !self.periods_loaded {
            self.reload_periods();
        }
        let mut import_preview = false;
        let mut import_now = false;
        ui.add_space(6.0);
        if theme::link(ui, "import from pomofocus", 17.0, theme::RED).clicked() {
            self.import_open = !self.import_open;
        }
        if self.import_open {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.add(
                    egui::TextEdit::multiline(&mut self.import_text)
                        .hint_text("23-Sep-2026\n14:39 ~ 16:56\nclickhouse\n113")
                        .desired_width((ui.available_width() - 24.0).max(200.0))
                        .desired_rows(5)
                        .font(theme::regular_font(13.0)),
                );
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                if theme::link(ui, "preview", 13.0, theme::RED).clicked() {
                    import_preview = true;
                }
                if theme::link(ui, "import", 13.0, theme::RED).clicked() {
                    import_now = true;
                }
            });
            if !self.import_msg.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(theme::tc(self.import_msg.clone(), 12.0, theme::MUTED));
                });
            }
            ui.add_space(8.0);
        }

        ui.add_space(16.0);
        if theme::link(ui, "periods", 17.0, theme::RED).clicked() {
            self.periods_open = !self.periods_open;
        }
        if self.periods_open {
            ui.add_space(8.0);
            let mut delete: Option<i64> = None;
            egui::Grid::new("log-table")
                .num_columns(5)
                .spacing([25.0, 3.0])
                .striped(false)
                .show(ui, |ui| {
                    for h in ["date", "range", "category", "min", ""] {
                        ui.label(theme::i(h, 11.0).color(theme::MUTED));
                    }
                    ui.end_row();
                    for p in &self.periods {
                        let date = p.started_at.get(..10).unwrap_or(&p.started_at);
                        let start = p.started_at.get(11..16).unwrap_or("");
                        let end = p.ended_at.get(11..16).unwrap_or("");
                        ui.label(theme::t(date, 13.0).color(theme::GOLD));
                        ui.label(theme::t(format!("{start} ~ {end}"), 13.0).color(theme::GOLD));
                        ui.label(
                            theme::i(
                                p.category_name.clone().unwrap_or_else(|| "Unallocated".into()),
                                13.0,
                            )
                            .color(theme::MUTED),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(theme::t(p.minutes.to_string(), 13.0).color(theme::BRIGHT));
                        });
                        let (rect, resp) = ui.allocate_exact_size(egui::Vec2::new(12.0, 14.0), egui::Sense::click());
                        let color = if resp.hovered() { theme::RED_HOT } else { theme::FAINT };
                        theme::paint_x(ui.painter(), rect.center(), 7.0, color);
                        if resp.clicked() {
                            delete = Some(p.id);
                        }
                        ui.end_row();
                    }
                });
            if self.periods.is_empty() {
                theme::empty_note(ui, "no periods", 16.0);
            }
            if let Some(id) = delete {
                let _ = self.db.delete_period(id);
                self.reload_periods();
            }
        }

        if import_preview || import_now {
            let text = self.import_text.clone();
            if text.trim().is_empty() {
                self.import_msg = "nothing to import".into();
            } else {
                match self.db.import_pomofocus(&text, import_preview) {
                    Ok(res) => {
                        if import_preview {
                            let mut out = format!("parsed {} rows.\n", res.parsed);
                            for r in &res.preview {
                                out.push_str(&format!(
                                    "{}  {}~{}  {:<18} {}m\n",
                                    &r.start[..10],
                                    &r.start[11..16],
                                    &r.end[11..16],
                                    r.project,
                                    r.minutes
                                ));
                            }
                            if res.parsed > res.preview.len() {
                                out.push_str(&format!(
                                    "... and {} more",
                                    res.parsed - res.preview.len()
                                ));
                            }
                            self.import_msg = out;
                        } else {
                            self.import_msg = format!(
                                "imported {}, skipped {} (duplicates) of {} parsed.",
                                res.imported, res.skipped, res.parsed
                            );
                            self.reload_periods();
                        }
                    }
                    Err(e) => self.import_msg = e.to_string(),
                }
            }
        }
    }

    pub fn export_card(&mut self, kind: lockin_core::CardKind) {
        match self.db.card_png(&kind) {
            Ok(bytes) => {
                let dir = dirs::cache_dir()
                    .unwrap_or_else(std::env::temp_dir)
                    .join("lockin");
                let _ = std::fs::create_dir_all(&dir);
                let name = match &kind {
                    lockin_core::CardKind::Day => "card-today.png".to_string(),
                    lockin_core::CardKind::Week => "card-week.png".to_string(),
                    lockin_core::CardKind::Month { year, month } => {
                        format!("card-{year}-{month:02}.png")
                    }
                };
                let path = dir.join(name);
                if std::fs::write(&path, bytes).is_ok() {
                    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
                }
            }
            Err(e) => self.toast(format!("export failed: {e}")),
        }
    }

    // keep imports used while later views are filled in
    #[allow(dead_code)]
    fn _layout(_: Layout, _: Align) {}
}


pub fn fmt_date(iso: &str) -> String {
    if iso.len() < 10 {
        return iso.to_string();
    }
    chrono::NaiveDate::parse_from_str(&iso[..10], "%Y-%m-%d")
        .map(|d| d.format("%b %d").to_string())
        .unwrap_or_else(|_| iso[..10].to_string())
}

impl App {
    pub fn view_summary_placeholder(&mut self, _ui: &mut Ui) {}
}


fn parse_month(value: &str) -> Result<(i32, u32), ()> {
    let (y, m) = value.split_once('-').ok_or(())?;
    Ok((y.parse().map_err(|_| ())?, m.parse().map_err(|_| ())?))
}

fn pretty_month(value: &str) -> String {
    const NAMES: [&str; 12] = [
        "january", "february", "march", "april", "may", "june", "july", "august", "september",
        "october", "november", "december",
    ];
    match parse_month(value) {
        Ok((y, m)) if (1..=12).contains(&m) => format!("{} {y}", NAMES[m as usize - 1]),
        _ => value.to_string(),
    }
}


fn popup_close(ui: &mut Ui) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::new(16.0, 16.0), egui::Sense::click());
    let color = if response.hovered() { theme::RED_HOT } else { theme::MUTED };
    theme::paint_x(ui.painter(), rect.center(), 8.0, color);
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}
