use std::slice;
use std::ops::{Add, Sub};
use std::f32::consts::PI;
use std::cmp::max;
use rand::{Rand, Rng};
use num::{Integer, Float};

/// Hex grid geometry for vectors.
pub trait HexGeom {
    /// Hex distance represented by a vector.
    fn hex_dist(&self) -> i32;
}

impl<T: AsRef<[i32; 2]>> HexGeom for T {
    fn hex_dist(&self) -> i32 {
        let v = self.as_ref();
        let xd = v[0];
        let yd = v[1];
        if xd.signum() == yd.signum() {
            max(xd.abs(), yd.abs())
        } else {
            xd.abs() + yd.abs()
        }
    }
}

/// Hex grid directions.
#[derive(Copy, Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum Dir6 {
    North = 0,
    NorthEast,
    SouthEast,
    South,
    SouthWest,
    NorthWest,
}

impl Dir6 {
    /// Convert a vector into the closest hex direction.
    ///
    /// ```notrust
    ///        *0*       *1*
    ///           \ 14 15 | 00 01
    ///           13\     |      02
    ///               \   |
    ///         12      \ |        03
    ///     *5* ----------O-X------- *2*
    ///         11        Y \      04
    ///                   |   \
    ///           10      |     \05
    ///             09 08 | 07 06 \
    ///                  *4*       *3*
    ///
    /// The hexadecants (00 to 15) and the hex
    /// directions (*0* to *5*) around the origin.
    /// ```
    ///
    /// Vectors that are in a space between two hex direction vectors are
    /// rounded to a hexadecant, then assigned the hex direction whose vector
    /// is nearest to that hexadecant.
    pub fn from_v2<V: Into<[i32; 2]>>(v: V) -> Dir6 {
        let v = v.into();

        let hexadecant = {
            let width = PI / 8.0;
            let mut radian = (v[0] as f32).atan2(-v[1] as f32);
            if radian < 0.0 {
                radian += 2.0 * PI
            }
            (radian / width).floor() as i32
        };

        Dir6::from_int(match hexadecant {
            13 | 14 => 0,
            15 | 0 | 1 => 1,
            2 | 3 | 4 => 2,
            5 | 6 => 3,
            7 | 8 | 9 => 4,
            10 | 11 | 12 => 5,
            _ => panic!("Bad hexadecant"),
        })
    }

    /// Convert an integer to a hex dir using modular arithmetic.
    pub fn from_int(i: i32) -> Dir6 {
        DIRS[i.mod_floor(&6) as usize]
    }

    /// Convert a hex dir into the corresponding unit vector.
    pub fn to_v2(&self) -> [i32; 2] {
        [[-1, -1], [0, -1], [1, 0], [1, 1], [0, 1], [-1, 0]][*self as usize]
    }

    /// Iterate through the six hex dirs in the standard order.
    pub fn iter() -> slice::Iter<'static, Dir6> {
        DIRS.iter()
    }
}

impl Add<i32> for Dir6 {
    type Output = Dir6;
    fn add(self, other: i32) -> Dir6 {
        Dir6::from_int(self as i32 + other)
    }
}

impl Sub<i32> for Dir6 {
    type Output = Dir6;
    fn sub(self, other: i32) -> Dir6 {
        Dir6::from_int(self as i32 - other)
    }
}

impl Rand for Dir6 {
    fn rand<R: Rng>(rng: &mut R) -> Dir6 {
        Dir6::from_int(rng.gen_range(0, 6))
    }
}

static DIRS: [Dir6; 6] = [Dir6::North,
                          Dir6::NorthEast,
                          Dir6::SouthEast,
                          Dir6::South,
                          Dir6::SouthWest,
                          Dir6::NorthWest];

/// Hex grid directions with transitional directions.
#[derive(Copy, Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum Dir12 {
    North = 0,
    NorthNorthEast,
    NorthEast,
    East,
    SouthEast,
    SouthSouthEast,
    South,
    SouthSouthWest,
    SouthWest,
    West,
    NorthWest,
    NorthNorthWest,
}

impl Dir12 {
    /// If there is exactly one cluster of neighbors in the neighbor mask,
    /// return a direction pointing away from that cluster.
    pub fn away_from(neighbors: &[bool; 6]) -> Option<Dir12> {
        use std::mem;

        let (begin, end) = match find_cluster(neighbors) {
            Some((a, b)) => (a, b),
            None => return None,
        };

        if !is_single_cluster(neighbors, begin, end) {
            return None;
        }

        let cluster_size = if end < begin {
            end + 6 - begin
        } else {
            end - begin
        };
        assert!(cluster_size > 0);

        // Dir12 in use from here on.
        let center_dir = begin * 2 + (cluster_size - 1);
        let away_dir: u8 = ((center_dir + 6) % 12) as u8;
        assert!(away_dir < 12);

        // XXX: Unsafe because I'm too lazy to do int conversion func by hand.
        return Some(unsafe { mem::transmute(away_dir) });

        fn find_cluster(neighbors: &[bool; 6]) -> Option<(usize, usize)> {
            // Start of the active cluster, inclusive.
            let mut cluster_start = None;
            // End of the active cluster, exclusive.
            let mut cluster_end = None;

            for i in 0..6 {
                if cluster_start.is_none() && neighbors[i] &&
                   !neighbors[(i + 5) % 6] {
                    cluster_start = Some(i);
                }

                if cluster_end.is_none() && !neighbors[i] &&
                   neighbors[(i + 5) % 6] {
                    cluster_end = Some(i);
                }
            }

            if cluster_start.is_none() {
                return None;
            }

            assert!(cluster_end.is_some()); // Must be some if start is some.

            Some((cluster_start.unwrap(), cluster_end.unwrap()))
        }

        fn is_single_cluster(neighbors: &[bool; 6],
                             start: usize,
                             end: usize)
                             -> bool {
            let mut in_cluster = true;

            for i in 0..6 {
                if (start + i) % 6 == end {
                    in_cluster = false;
                }

                if neighbors[(start + i) % 6] != in_cluster {
                    return false;
                }
            }

            true
        }
    }
}

/// Field of view iterator for a hexagonal map.
///
/// Takes a function that
/// indicates opaque cells and yields the visible locations from around the
/// origin up to the given radius.
pub struct HexFov<F> {
    /// Predicate for whether a given point will block the field of view.
    is_opaque: F,
    range: u32,
    stack: Vec<Sector>,
    fake_isometric_hack: bool,
    /// Extra values generated by special cases.
    side_channel: Vec<[i32; 2]>,
}

impl<F> HexFov<F> where F: Fn([i32; 2]) -> bool
{
    pub fn new(is_opaque: F, range: u32) -> HexFov<F> {
        let init_group = is_opaque(Dir6::from_int(0).to_v2());
        HexFov {
            is_opaque: is_opaque,
            range: range,
            stack: vec![Sector {
                            begin: PolarPoint::new(0.0, 1),
                            pt: PolarPoint::new(0.0, 1),
                            end: PolarPoint::new(6.0, 1),
                            group_opaque: init_group,
                        }],
            fake_isometric_hack: false,
            // The FOV algorithm will not generate the origin point, so we use
            // the side channel to explicitly add it in the beginning.
            side_channel: vec![[0, 0]],
        }
    }

    /// Make wall tiles in acute corners visible when running the algorithm so
    /// that the complete wall rectangle of fake-isometric rooms will appear
    /// in the FOV.
    pub fn fake_isometric(mut self) -> HexFov<F> {
        self.fake_isometric_hack = true;
        self
    }
}

impl<F> Iterator for HexFov<F> where F: Fn([i32; 2]) -> bool
{
    type Item = [i32; 2];
    fn next(&mut self) -> Option<[i32; 2]> {
        if let Some(ret) = self.side_channel.pop() {
            return Some(ret);
        }

        if let Some(mut current) = self.stack.pop() {
            if current.pt.is_below(current.end) {
                let pos = current.pt.to_v2();
                let current_opaque = (self.is_opaque)(pos);

                // Terrain opacity changed, branch out.
                if current_opaque != current.group_opaque {
                    // Add the rest of this sector with the new opacity.
                    self.stack.push(Sector {
                        begin: current.pt,
                        pt: current.pt,
                        end: current.end,
                        group_opaque: current_opaque,
                    });

                    // If this was a visible sector and we're below range, branch
                    // out further.
                    if !current.group_opaque &&
                       current.begin.radius < self.range {
                        self.stack.push(Sector {
                            begin: current.begin.further(),
                            pt: current.begin.further(),
                            end: current.pt.further(),
                            group_opaque: (self.is_opaque)(current.begin.further().to_v2()),
                        });
                    }
                    return self.next();
                }

                // Hack for making acute corner tiles of fake-isometric rooms
                // visible.

                // XXX: Side cells should only be visible with wallform tiles,
                // but the FOV API can't currently distinguish between
                // wallform and blockform FOV blockers.
                if self.fake_isometric_hack {
                    if let Some(side_pt) = current.pt.side_point() {
                        // Only do this if both the front tiles and the target
                        // tile are opaque.
                        let next = current.pt.next();
                        if next.is_below(current.end) && current.group_opaque &&
                           (self.is_opaque)(next.to_v2()) &&
                           (self.is_opaque)(side_pt) &&
                           current.begin.radius < self.range {
                            self.side_channel.push(side_pt);
                        }
                    }
                }

                // Proceed along the current sector.
                current.pt = current.pt.next();
                self.stack.push(current);
                return Some(pos);
            } else {
                // Hit the end of the sector.

                // If this was a visible sector and we're below range, branch
                // out further.
                if !current.group_opaque && current.begin.radius < self.range {
                    self.stack.push(Sector {
                        begin: current.begin.further(),
                        pt: current.begin.further(),
                        end: current.end.further(),
                        group_opaque: (self.is_opaque)(current.begin
                                                              .further()
                                                              .to_v2()),
                    });
                }

                self.next()
            }
        } else {
            None
        }
    }
}

struct Sector {
    /// Start point of current sector.
    begin: PolarPoint,
    /// Point currently being processed.
    pt: PolarPoint,
    /// End point of current sector.
    end: PolarPoint,
    /// Currently iterating through a sequence of opaque cells.
    group_opaque: bool,
}

/// Points on a hex circle expressed in polar coordinates.
#[derive(Copy, Clone, PartialEq)]
struct PolarPoint {
    pos: f32,
    radius: u32,
}

impl PolarPoint {
    pub fn new(pos: f32, radius: u32) -> PolarPoint {
        PolarPoint {
            pos: pos,
            radius: radius,
        }
    }
    /// Index of the discrete hex cell along the circle that corresponds to this point.
    fn winding_index(self) -> i32 {
        (self.pos + 0.5).floor() as i32
    }

    pub fn is_below(self, other: PolarPoint) -> bool {
        self.winding_index() < other.end_index()
    }
    fn end_index(self) -> i32 {
        (self.pos + 0.5).ceil() as i32
    }

    pub fn to_v2(self) -> [i32; 2] {
        if self.radius == 0 {
            return [0, 0];
        }
        let index = self.winding_index();
        let sector = index.mod_floor(&(self.radius as i32 * 6)) /
                     self.radius as i32;
        let offset = index.mod_floor(&(self.radius as i32));

        let rod = Dir6::from_int(sector).to_v2();
        let tangent = Dir6::from_int((sector + 2) % 6).to_v2();

        [rod[0] * self.radius as i32 + tangent[0] * offset,
         rod[1] * self.radius as i32 + tangent[1] * offset]
    }

    /// If this point and the next point are adjacent vertically (along the xy
    /// axis), return a tuple of the point outside of the circle between the
    /// two points.
    ///
    /// This is a helper function for the FOV special case where acute corners
    /// of fake isometric rooms are marked visible even though strict hex FOV
    /// logic would keep them unseen.
    pub fn side_point(self) -> Option<[i32; 2]> {
        let next = self.next();
        let a = self.to_v2();
        let b = next.to_v2();

        if b[0] == a[0] + 1 && b[1] == a[1] + 1 {
            // Going down the right rim.
            Some([a[0] + 1, a[1]])
        } else if b[0] == a[0] - 1 && b[1] == a[1] - 1 {
            // Going up the left rim.
            Some([a[0] - 1, a[1]])
        } else {
            None
        }
    }

    /// The point corresponding to this one on the hex circle with radius +1.
    pub fn further(self) -> PolarPoint {
        PolarPoint::new(self.pos * (self.radius + 1) as f32 /
                        self.radius as f32,
                        self.radius + 1)
    }

    /// The point next to this one along the hex circle.
    pub fn next(self) -> PolarPoint {
        PolarPoint::new((self.pos + 0.5).floor() + 0.5, self.radius)
    }
}

#[cfg(test)]
mod test {
    use super::Dir6;
    use super::Dir6::*;
    use super::Dir12;

    #[test]
    fn test_dir6() {
        assert_eq!(North, Dir6::from_int(0));
        assert_eq!(NorthWest, Dir6::from_int(-1));
        assert_eq!(NorthWest, Dir6::from_int(5));
        assert_eq!(NorthEast, Dir6::from_int(1));

        assert_eq!(NorthEast, Dir6::from_v2([20i32, -21i32]));
        assert_eq!(SouthEast, Dir6::from_v2([20, -10]));
        assert_eq!(North, Dir6::from_v2([-10, -10]));
        assert_eq!(South, Dir6::from_v2([1, 1]));

        for i in 0..6 {
            let d = Dir6::from_int(i);
            let v = d.to_v2();
            let v1 = Dir6::from_int(i - 1).to_v2();
            let v2 = Dir6::from_int(i + 1).to_v2();

            // Test static iter
            assert_eq!(Some(d), Dir6::iter().nth(i as usize).map(|&x| x));

            // Test vector mapping.
            assert_eq!(d, Dir6::from_v2(v));

            // Test opposite dir vector mapping.
            assert_eq!(Dir6::from_int(i + 3), Dir6::from_v2([-v[0], -v[1]]));

            // Test approximation of longer vectors.
            assert_eq!(d, Dir6::from_v2([v[0] * 3, v[1] * 3]));
            assert_eq!(d, Dir6::from_v2([v[0] * 3 + v1[0], v[1] * 3 + v1[1]]));
            assert_eq!(d, Dir6::from_v2([v[0] * 3 + v2[0], v[1] * 3 + v2[1]]));
        }
    }

    #[test]
    fn test_dir12() {
        assert_eq!(None,
                   Dir12::away_from(&[false, false, false, false, false,
                                      false]));
        assert_eq!(None,
                   Dir12::away_from(&[true, true, true, true, true, true]));
        assert_eq!(None,
                   Dir12::away_from(&[false, true, false, false, true, false]));
        assert_eq!(None,
                   Dir12::away_from(&[true, true, false, true, false, false]));
        assert_eq!(None,
                   Dir12::away_from(&[true, false, true, false, true, false]));
        assert_eq!(Some(Dir12::South),
                   Dir12::away_from(&[true, false, false, false, false,
                                      false]));
        assert_eq!(Some(Dir12::East),
                   Dir12::away_from(&[true, false, false, true, true, true]));
        assert_eq!(Some(Dir12::SouthSouthWest),
                   Dir12::away_from(&[true, true, false, false, false, false]));
        assert_eq!(Some(Dir12::SouthWest),
                   Dir12::away_from(&[true, true, true, false, false, false]));
        assert_eq!(Some(Dir12::SouthSouthEast),
                   Dir12::away_from(&[true, true, false, false, true, true]));
    }
}