//! A horizontal scroller whose content fades out at an edge with more
//! beyond it, so a row cut off by the frame says there is more to see
//! (Settings → Message List → Columns, #334).
//!
//! The fade is a mask over the content rather than a gradient painted on
//! top, so it reads the same on any background, the highlight of a drop
//! zone under a drag included. The widget's own frame (its CSS border and
//! padding) is not faded.

use std::cell::RefCell;

use adw::prelude::*;
use gtk::{gdk, glib, graphene, gsk, subclass::prelude::*};

/// Width of the fade at an edge with more beyond it.
const FADE: f32 = 32.0;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct FadeScroll {
        pub scrolled: RefCell<Option<gtk::ScrolledWindow>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FadeScroll {
        const NAME: &'static str = "HylkiFadeScroll";
        type Type = super::FadeScroll;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for FadeScroll {
        fn dispose(&self) {
            if let Some(s) = self.scrolled.take() {
                s.unparent();
            }
        }
    }

    impl WidgetImpl for FadeScroll {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match self.scrolled.borrow().as_ref() {
                Some(s) => s.measure(orientation, for_size),
                None => (0, 0, -1, -1),
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(s) = self.scrolled.borrow().as_ref() {
                s.allocate(width, height, baseline, None);
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let Some(scrolled) = self.scrolled.borrow().clone() else { return };
            let adj = scrolled.hadjustment();
            let before = (adj.value() - adj.lower()) as f32;
            let after = (adj.upper() - adj.page_size() - adj.value()) as f32;
            let Some(bounds) = scrolled.compute_bounds(&*obj) else { return };
            let w = bounds.width();
            if (before <= 0.0 && after <= 0.0) || w <= 0.0 {
                obj.snapshot_child(&scrolled, snapshot);
                return;
            }
            // Each fade is as deep as the content hidden past its edge, up
            // to FADE, so it comes and goes smoothly at the ends.
            let left = (before / FADE).clamp(0.0, 1.0);
            let right = (after / FADE).clamp(0.0, 1.0);
            let edge = (FADE / w).min(0.5);
            let stop = |at: f32, alpha: f32| gsk::ColorStop::new(at, gdk::RGBA::new(0.0, 0.0, 0.0, alpha));
            snapshot.push_mask(gsk::MaskMode::Alpha);
            snapshot.append_linear_gradient(
                &bounds,
                &graphene::Point::new(bounds.x(), 0.0),
                &graphene::Point::new(bounds.x() + w, 0.0),
                &[stop(0.0, 1.0 - left), stop(edge, 1.0), stop(1.0 - edge, 1.0), stop(1.0, 1.0 - right)],
            );
            snapshot.pop();
            obj.snapshot_child(&scrolled, snapshot);
            snapshot.pop();
        }
    }
}

glib::wrapper! {
    pub struct FadeScroll(ObjectSubclass<imp::FadeScroll>)
        @extends gtk::Widget;
}

impl FadeScroll {
    /// Scroll `child` sideways; it is never scrolled up and down.
    pub fn new(child: &impl IsA<gtk::Widget>) -> Self {
        let obj: Self = glib::Object::new();
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        scrolled.set_child(Some(child));
        scrolled.set_parent(&obj);
        let adj = scrolled.hadjustment();
        let weak = obj.downgrade();
        adj.connect_value_changed(move |_| {
            if let Some(o) = weak.upgrade() {
                o.queue_draw();
            }
        });
        let weak = obj.downgrade();
        adj.connect_changed(move |_| {
            if let Some(o) = weak.upgrade() {
                o.queue_draw();
            }
        });
        obj.imp().scrolled.replace(Some(scrolled));
        obj
    }

    pub fn hadjustment(&self) -> gtk::Adjustment {
        self.imp().scrolled.borrow().as_ref().map(|s| s.hadjustment()).unwrap_or_default()
    }
}
