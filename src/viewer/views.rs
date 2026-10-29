//! Named views: cameras saved under a name with the document, to come back
//! to the same framing (FEAT-C4).

use glam::{Quat, Vec3};

use super::Viewer;
use super::camera::Pose;
use crate::settings::{NamedView, Projection};

impl Viewer {
    /// Saves where the camera is heading as a named view and returns its
    /// name: `name` trimmed, or the first free "View N" without one. A view
    /// with the same name is replaced in place.
    pub fn save_view(&mut self, name: Option<&str>, projection: Projection) -> String {
        let name = match name.map(str::trim).filter(|name| !name.is_empty()) {
            Some(name) => name.to_owned(),
            None => (1..)
                .map(|n| format!("View {n}"))
                .find(|name| !self.named_views.iter().any(|view| &view.name == name))
                .expect("a free name"),
        };
        let view = capture(name.clone(), self.settled_pose(), projection);
        match self.named_views.iter_mut().find(|view| view.name == name) {
            Some(existing) => *existing = view,
            None => self.named_views.push(view),
        }
        name
    }

    /// Stores where the camera is heading in the view at `index`.
    pub fn update_view(&mut self, index: usize, projection: Projection) -> Option<&str> {
        let pose = self.settled_pose();
        let view = self.named_views.get_mut(index)?;
        *view = capture(std::mem::take(&mut view.name), pose, projection);
        Some(&view.name)
    }

    pub fn delete_view(&mut self, index: usize) -> Option<NamedView> {
        (index < self.named_views.len()).then(|| self.named_views.remove(index))
    }

    /// Moves the camera to the view at `index` and returns the projection
    /// it was saved with; `None` for an unknown index or a damaged entry
    /// (the settings file is user-editable).
    pub fn go_to_view(&mut self, index: usize, animate: bool, now: f64) -> Option<Projection> {
        let view = self.named_views.get(index)?;
        let (pose, projection) = (pose_of(view)?, view.projection);
        self.animate_to(pose, animate, now);
        Some(projection)
    }
}

fn capture(name: String, pose: Pose, projection: Projection) -> NamedView {
    NamedView {
        name,
        target: pose.target.to_array(),
        rotation: pose.rotation.normalize().to_array(),
        extent: pose.extent,
        projection,
    }
}

/// The camera pose of a saved view, if its numbers make one.
fn pose_of(view: &NamedView) -> Option<Pose> {
    let target = Vec3::from_array(view.target);
    let rotation = Quat::from_array(view.rotation);
    let valid = target.is_finite()
        && rotation.is_finite()
        && rotation.length() > 1e-3
        && view.extent.is_finite()
        && view.extent > 0.0;
    valid.then(|| Pose {
        target,
        rotation: rotation.normalize(),
        extent: view.extent,
    })
}

#[cfg(test)]
#[path = "../../tests/rust/viewer/test_views.rs"]
mod tests;
