//! A dense 2D grid backed by a flat, row-major `Vec`.
//!
//! Every per-cell buffer in this crate — heightmap, flow, river and lake
//! masks — is a [`Grid`]. It keeps the `(x, y)` ↔ `y * width + x` mapping
//! in one place instead of scattering index arithmetic across modules,
//! while still exposing the flat buffer for zero-copy hand-off to FFI,
//! GPUs or file formats.

use std::ops::{Index, IndexMut};

/// A `width × height` grid of `T`, stored row-major.
///
/// Indexing is `(x, y)`: `grid[(x, y)]` reads the cell at column `x`,
/// row `y`.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid<T> {
    width: usize,
    height: usize,
    data: Vec<T>,
}

impl<T: Clone> Grid<T> {
    /// Creates a grid filled with `value`.
    ///
    /// # Panics
    /// Panics if either dimension is `0`.
    pub fn new(width: usize, height: usize, value: T) -> Self {
        assert!(width > 0 && height > 0, "grid dimensions must be positive");
        Self {
            width,
            height,
            data: vec![value; width * height],
        }
    }
}

impl<T> Grid<T> {
    /// Wraps an existing row-major buffer.
    ///
    /// # Panics
    /// Panics if `data.len() != width * height` or either dimension is `0`.
    pub fn from_vec(width: usize, height: usize, data: Vec<T>) -> Self {
        assert!(width > 0 && height > 0, "grid dimensions must be positive");
        assert_eq!(
            data.len(),
            width * height,
            "buffer length must equal width * height"
        );
        Self {
            width,
            height,
            data,
        }
    }

    /// Grid width in cells.
    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Grid height in cells.
    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Total number of cells (`width * height`).
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Always `false`: grids have positive dimensions by construction.
    #[inline]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Flat index of `(x, y)`.
    ///
    /// Callers holding flat indices (e.g. from [`Self::as_slice`]) can map
    /// back with [`Self::coords`].
    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        debug_assert!(x < self.width && y < self.height);
        y * self.width + x
    }

    /// `(x, y)` coordinates of a flat index.
    #[inline]
    pub fn coords(&self, idx: usize) -> (usize, usize) {
        (idx % self.width, idx / self.width)
    }

    /// The underlying row-major buffer.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    /// The underlying row-major buffer, mutably.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }

    /// Consumes the grid, returning the row-major buffer.
    pub fn into_vec(self) -> Vec<T> {
        self.data
    }

    /// Iterates over all cells in row-major order.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }

    /// Iterates mutably over all cells in row-major order.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.data.iter_mut()
    }
}

impl<T> Index<(usize, usize)> for Grid<T> {
    type Output = T;

    #[inline]
    fn index(&self, (x, y): (usize, usize)) -> &T {
        &self.data[self.idx(x, y)]
    }
}

impl<T> IndexMut<(usize, usize)> for Grid<T> {
    #[inline]
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut T {
        let idx = self.idx(x, y);
        &mut self.data[idx]
    }
}

impl<'a, T> IntoIterator for &'a Grid<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_between_coords_and_flat_index() {
        let grid = Grid::new(7, 5, 0u8);
        for y in 0..5 {
            for x in 0..7 {
                assert_eq!(grid.coords(grid.idx(x, y)), (x, y));
            }
        }
    }

    #[test]
    fn indexing_is_row_major() {
        let mut grid = Grid::new(3, 2, 0i32);
        grid[(2, 1)] = 42;
        assert_eq!(grid.as_slice(), &[0, 0, 0, 0, 0, 42]);
    }

    #[test]
    #[should_panic]
    fn from_vec_rejects_wrong_length() {
        let _ = Grid::from_vec(2, 2, vec![0u8; 3]);
    }

    #[test]
    #[should_panic]
    fn zero_dimensions_are_rejected() {
        let _ = Grid::new(0, 4, 0u8);
    }
}
