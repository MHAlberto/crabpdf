//! Bounded display snapshots during annotation changes. Native caches remain strict.
use crate::{cache::ByteCache, document::RenderKey};
use std::sync::Arc;

pub(super) fn preview<V>(
    cache: &mut ByteCache<RenderKey, V>,
    target: RenderKey,
) -> Option<(u64, Arc<V>)> {
    cache
        .best_match(|key| {
            (key.document_id == target.document_id
                && key.page == target.page
                && key.rotation == target.rotation
                && key.colors == target.colors
                && key.region.is_none()
                && key.revision <= target.revision)
                .then_some(key.revision)
        })
        .map(|(key, value)| (key.revision, value))
}

pub(super) fn stale_tile<V>(
    cache: &mut ByteCache<RenderKey, V>,
    target: RenderKey,
    preview_revision: Option<u64>,
) -> Option<Arc<V>> {
    cache
        .best_match(|key| {
            (key.revision < target.revision
                && key.revision >= preview_revision.unwrap_or(0)
                && RenderKey {
                    revision: target.revision,
                    ..*key
                } == target)
                .then_some(key.revision)
        })
        .map(|(_, value)| value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crabpdf_core::document::{PageColors, RasterRegion, Zoom};
    fn key() -> RenderKey {
        RenderKey {
            document_id: 7,
            revision: 2,
            page: 3,
            zoom: Zoom::Percent(100),
            viewport_width: 900,
            viewport_height: 700,
            density_milli: 1000,
            bitmap_side_limit: 4096,
            rotation: 0,
            colors: PageColors::Original,
            region: None,
        }
    }
    #[test]
    fn annotations_keep_a_bounded_snapshot_and_prefer_the_newest_revision() {
        let mut cache = ByteCache::new(12);
        let target = key();
        let old = RenderKey {
            revision: 0,
            ..target
        };
        cache.insert(old, Arc::new("old"), 4);
        assert_eq!(*preview(&mut cache, target).unwrap().1, "old");
        assert!(!cache.contains(&target));
        cache.insert(
            RenderKey {
                revision: 1,
                ..target
            },
            Arc::new("recent"),
            4,
        );
        cache.get(&old); // Touching an older image cannot outrank newer content.
        assert_eq!(*preview(&mut cache, target).unwrap().1, "recent");
        cache.insert(target, Arc::new("current"), 4);
        assert_eq!(*preview(&mut cache, target).unwrap().1, "current");
        cache.insert(RenderKey { page: 4, ..target }, Arc::new("neighbor"), 4);
        assert_eq!(cache.bytes(), 12);
        assert_eq!(*preview(&mut cache, target).unwrap().1, "current");
    }
    #[test]
    fn snapshots_never_cross_documents_pages_rotation_colors_or_future_revisions() {
        let mut cache = ByteCache::new(20);
        let target = key();
        for other in [
            RenderKey {
                document_id: 8,
                ..target
            },
            RenderKey { page: 4, ..target },
            RenderKey {
                rotation: 1,
                ..target
            },
            RenderKey {
                colors: PageColors::Inverted,
                ..target
            },
            RenderKey {
                revision: 3,
                ..target
            },
        ] {
            cache.insert(other, Arc::new(1), 4);
        }
        assert!(preview(&mut cache, target).is_none());
    }
    #[test]
    fn old_tiles_cannot_cover_a_new_overview_or_use_another_geometry() {
        let mut cache = ByteCache::new(16);
        let target = RenderKey {
            region: Some(RasterRegion {
                x: 0,
                y: 0,
                width: 1024,
                height: 1024,
            }),
            ..key()
        };
        cache.insert(
            RenderKey {
                revision: 1,
                ..target
            },
            Arc::new(1),
            4,
        );
        assert!(stale_tile(&mut cache, target, Some(0)).is_some());
        assert!(stale_tile(&mut cache, target, Some(2)).is_none());
        assert!(
            stale_tile(
                &mut cache,
                RenderKey {
                    density_milli: 2000,
                    ..target
                },
                None
            )
            .is_none()
        );
        assert!(
            stale_tile(
                &mut cache,
                RenderKey {
                    zoom: Zoom::Percent(200),
                    ..target
                },
                None
            )
            .is_none()
        );
    }
}
