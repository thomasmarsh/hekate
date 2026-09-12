//! Property-style robustness tests for the scenario boundary.
//!
//! Phase 1 Increment 1 gate: "Property/fuzz inputs within declared limits never
//! panic the parser or validator and either compile or return diagnostics."
//! These tests drive arbitrary text and arbitrary source structures (within
//! declared size limits) through the public parse, validate, and compile entry
//! points and assert that every outcome is a value rather than a panic.
//!
//! The generator is a deterministic xorshift PRNG so a failure reproduces from
//! its fixed seed without adding a dependency.

use tangle_model::{
    CompiledScenario, ConflictRegionSource, CrossingSource, DemandSource, MovementSource, PathEnd,
    PathSource, PointSource, PolygonSource, PopulationSource, PortalSource, ProfileRangeSource,
    ProfileSource, RouteShareSource, RuleKind, RuleSource, ScenarioSource, SignalColor,
    SignalHeadSource, SignalPhaseSource, SignalSource, SignalStateSource, parse_scenario_source,
    validate,
};

/// Declared generation limits: the boundary above which the gate makes no
/// promise, kept small so a failing case is easy to minimize by hand.
const MAX_COLLECTION: u32 = 4;
const MAX_POINTS: u32 = 6;
const MAX_COORD_M: f64 = 100.0;

/// A deterministic xorshift64 PRNG.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A value in `0..bound`; zero bounds produce zero.
    fn below(&mut self, bound: u32) -> u32 {
        (self.next_u64() % u64::from(bound.max(1))) as u32
    }

    /// A finite coordinate within `±MAX_COORD_M`.
    fn coord(&mut self) -> f64 {
        (self.next_u64() % 2001) as f64 / 10.0 - MAX_COORD_M
    }

    /// A strictly positive dimension.
    fn positive(&mut self) -> f64 {
        (self.next_u64() % 200) as f64 / 10.0 + 0.5
    }

    /// One identifier from a tiny pool, so duplicates and dangling references
    /// both occur frequently.
    fn id(&mut self) -> String {
        ["a", "b", "c", "d"][self.below(4) as usize].to_owned()
    }

    fn point(&mut self) -> PointSource {
        PointSource {
            x: self.coord(),
            y: self.coord(),
        }
    }

    fn polygon(&mut self) -> PolygonSource {
        PolygonSource {
            id: self.id(),
            points: (0..self.below(MAX_POINTS)).map(|_| self.point()).collect(),
        }
    }

    /// A finite positive profile range, occasionally inverted or non-positive
    /// so the validator branch is exercised.
    fn range(&mut self) -> ProfileRangeSource {
        let min = match self.below(4) {
            0 => 0.0,
            1 => -1.0,
            _ => self.positive(),
        };
        let max = if self.below(4) == 0 {
            min - 1.0
        } else {
            min + self.positive()
        };
        ProfileRangeSource { min, max }
    }
}

/// Arbitrary text must always parse or return a structural error, never panic.
#[test]
fn random_text_never_panics_the_parser() {
    let mut rng = Rng::new(0x5eed_1234);
    for _ in 0..4000 {
        let length = rng.below(96) as usize;
        let text: String = (0..length)
            .map(|_| char::from(rng.below(256) as u8))
            .collect();
        // The contract is only that the call returns.
        let _ = parse_scenario_source(&text);
    }
}

/// Build a structurally random, likely-invalid source within declared limits.
fn random_source(rng: &mut Rng) -> ScenarioSource {
    let paths: Vec<PathSource> = (0..rng.below(MAX_COLLECTION))
        .map(|_| PathSource {
            id: rng.id(),
            points: (0..rng.below(MAX_POINTS)).map(|_| rng.point()).collect(),
        })
        .collect();
    let portals: Vec<PortalSource> = (0..rng.below(MAX_COLLECTION))
        .map(|_| PortalSource {
            id: rng.id(),
            path: rng.id(),
            end: if rng.below(2) == 0 {
                PathEnd::Start
            } else {
                PathEnd::End
            },
            width_m: if rng.below(4) == 0 {
                0.0
            } else {
                rng.positive()
            },
        })
        .collect();
    let boundaries: Vec<PolygonSource> = (0..rng.below(MAX_COLLECTION))
        .map(|_| rng.polygon())
        .collect();
    let regions: Vec<PolygonSource> = (0..rng.below(MAX_COLLECTION))
        .map(|_| rng.polygon())
        .collect();
    let movements: Vec<MovementSource> = (0..rng.below(MAX_COLLECTION))
        .map(|_| MovementSource {
            id: rng.id(),
            from: rng.id(),
            to: rng.id(),
            path: rng.id(),
            priority: rng.below(3),
            // Occasionally negative or past the path end so the stop-line
            // diagnostic branch is exercised.
            stop_line_m: if rng.below(4) == 0 {
                -1.0
            } else {
                f64::from(rng.below(200))
            },
        })
        .collect();
    let crossings: Vec<CrossingSource> = (0..rng.below(MAX_COLLECTION))
        .map(|_| CrossingSource {
            id: rng.id(),
            region: rng.id(),
            movements: (0..rng.below(3)).map(|_| rng.id()).collect(),
        })
        .collect();
    let conflict_regions: Vec<ConflictRegionSource> = (0..rng.below(MAX_COLLECTION))
        .map(|_| ConflictRegionSource {
            id: rng.id(),
            points: (0..rng.below(MAX_POINTS)).map(|_| rng.point()).collect(),
            movements: (0..rng.below(3)).map(|_| rng.id()).collect(),
        })
        .collect();
    let rules: Vec<RuleSource> = (0..rng.below(MAX_COLLECTION))
        .map(|_| RuleSource {
            id: rng.id(),
            movement: rng.id(),
            kind: [
                RuleKind::Free,
                RuleKind::Yield,
                RuleKind::Stop,
                RuleKind::Signal,
            ][rng.below(4) as usize],
            signal: if rng.below(2) == 0 {
                None
            } else {
                Some(rng.id())
            },
        })
        .collect();
    let demand: Vec<DemandSource> = (0..rng.below(MAX_COLLECTION))
        .map(|_| DemandSource {
            id: rng.id(),
            portal: rng.id(),
            rate_vph: if rng.below(4) == 0 {
                0.0
            } else {
                rng.positive() * 100.0
            },
            routes: (0..rng.below(3))
                .map(|_| RouteShareSource {
                    movement: rng.id(),
                    weight: rng.positive(),
                })
                .collect(),
        })
        .collect();
    let signals: Vec<SignalSource> = (0..rng.below(MAX_COLLECTION))
        .map(|_| SignalSource {
            id: rng.id(),
            heads: (0..rng.below(MAX_COLLECTION))
                .map(|_| SignalHeadSource {
                    id: rng.id(),
                    movement: rng.id(),
                })
                .collect(),
            phases: (0..rng.below(MAX_COLLECTION))
                .map(|_| SignalPhaseSource {
                    duration_s: if rng.below(4) == 0 {
                        -1.0
                    } else {
                        rng.positive()
                    },
                    states: (0..rng.below(MAX_COLLECTION))
                        .map(|_| SignalStateSource {
                            head: rng.id(),
                            color: [SignalColor::Red, SignalColor::Yellow, SignalColor::Green]
                                [rng.below(3) as usize],
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();

    ScenarioSource {
        schema_version: if rng.below(8) == 0 { 2 } else { 1 },
        id: rng.id(),
        coordinate_system: tangle_model::CoordinateSystem {
            x: "east_m".to_owned(),
            y: "north_m".to_owned(),
        },
        paths,
        portals,
        boundaries,
        regions,
        movements,
        crossings,
        conflict_regions,
        rules,
        signals,
        demand,
        profiles: ProfileSource {
            speed_mps: rng.range(),
            length_m: rng.range(),
            width_m: rng.range(),
            time_gap_s: rng.range(),
            max_accel_mps2: rng.range(),
            comfortable_brake_mps2: rng.range(),
            compliance: rng.range(),
        },
        population: PopulationSource {
            vehicle_count: rng.below(8),
            vehicle_speed_mps: rng.positive(),
            vehicle_spacing_m: rng.positive(),
            vehicle_length_m: rng.positive(),
            vehicle_width_m: rng.positive(),
        },
    }
}

/// Arbitrary sources either compile or return diagnostics, never panic, and a
/// source that validates always compiles with the id map covering every
/// compiled collection.
#[test]
fn random_sources_compile_or_diagnose_without_panicking() {
    let mut rng = Rng::new(0xc0ff_ee00);
    let mut compiled = 0;
    for iteration in 0..3000 {
        // Interleave a referentially consistent source so both the diagnostic
        // and the compile branch are exercised by the same property.
        let source = if iteration % 8 == 0 {
            consistent_source(&mut rng)
        } else {
            random_source(&mut rng)
        };
        let diagnostics = validate(&source);
        if diagnostics.is_empty() {
            let scenario = CompiledScenario::compile(source).expect("a validated source compiles");
            let id_map = scenario.id_map();
            assert_eq!(id_map.paths().len(), scenario.paths().len());
            assert_eq!(id_map.portals().len(), scenario.portals().len());
            assert_eq!(id_map.boundaries().len(), scenario.boundaries().len());
            assert_eq!(id_map.regions().len(), scenario.regions().len());
            assert_eq!(id_map.movements().len(), scenario.movements().len());
            assert_eq!(id_map.crossings().len(), scenario.crossings().len());
            assert_eq!(
                id_map.conflict_regions().len(),
                scenario.conflict_regions().len()
            );
            assert_eq!(id_map.rules().len(), scenario.rules().len());
            assert_eq!(id_map.signals().len(), scenario.signals().len());
            assert_eq!(id_map.demand().len(), scenario.demand().len());
            compiled += 1;
        } else {
            assert!(CompiledScenario::compile(source).is_err());
        }
    }
    assert!(
        compiled > 0,
        "the generator never produced a compilable scenario"
    );
}

/// A referentially consistent source with every primitive present must
/// validate and compile, exercising the dense-id and derived-geometry paths
/// under randomized but legal geometry.
#[test]
fn consistent_sources_always_compile() {
    let mut rng = Rng::new(0xabcd_9876);
    for _ in 0..500 {
        let source = consistent_source(&mut rng);
        let diagnostics = validate(&source);
        assert!(
            diagnostics.is_empty(),
            "consistent source produced diagnostics: {diagnostics:?}"
        );
        let scenario = CompiledScenario::compile(source).expect("consistent source compiles");
        assert_eq!(scenario.movements().len(), 2);
        let signal = scenario.signals().first().expect("signal compiles");
        assert_eq!(signal.heads().len(), 2);
        assert!(signal.cycle_s() > 0.0);
        assert!(scenario.boundaries()[0].polygon().area() > 0.0);
    }
}

/// Two straight paths, one crossing region, one conflict region, two
/// movements, and a two-phase signal that never shows both movements green.
fn consistent_source(rng: &mut Rng) -> ScenarioSource {
    let paths = vec![
        PathSource {
            id: "ew".to_owned(),
            points: vec![
                PointSource { x: -40.0, y: 0.0 },
                PointSource { x: 40.0, y: 0.0 },
            ],
        },
        PathSource {
            id: "ns".to_owned(),
            points: vec![
                PointSource { x: 0.0, y: -40.0 },
                PointSource { x: 0.0, y: 40.0 },
            ],
        },
    ];
    let portals = vec![
        PortalSource {
            id: "west".to_owned(),
            path: "ew".to_owned(),
            end: PathEnd::Start,
            width_m: rng.positive(),
        },
        PortalSource {
            id: "east".to_owned(),
            path: "ew".to_owned(),
            end: PathEnd::End,
            width_m: rng.positive(),
        },
        PortalSource {
            id: "south".to_owned(),
            path: "ns".to_owned(),
            end: PathEnd::Start,
            width_m: rng.positive(),
        },
        PortalSource {
            id: "north".to_owned(),
            path: "ns".to_owned(),
            end: PathEnd::End,
            width_m: rng.positive(),
        },
    ];
    let half = 1.0 + f64::from(rng.below(50)) / 10.0;
    let square = |id: &str, r: f64| PolygonSource {
        id: id.to_owned(),
        points: vec![
            PointSource { x: -r, y: -r },
            PointSource { x: r, y: -r },
            PointSource { x: r, y: r },
            PointSource { x: -r, y: r },
        ],
    };
    ScenarioSource {
        schema_version: 1,
        id: "fuzz_four_leg".to_owned(),
        coordinate_system: tangle_model::CoordinateSystem {
            x: "east_m".to_owned(),
            y: "north_m".to_owned(),
        },
        paths,
        portals,
        boundaries: vec![square("world", 50.0)],
        regions: vec![square("area", half)],
        movements: vec![
            MovementSource {
                id: "ew_through".to_owned(),
                from: "west".to_owned(),
                to: "east".to_owned(),
                path: "ew".to_owned(),
                priority: rng.below(3),
                stop_line_m: 0.0,
            },
            MovementSource {
                id: "ns_through".to_owned(),
                from: "south".to_owned(),
                to: "north".to_owned(),
                path: "ns".to_owned(),
                priority: rng.below(3),
                stop_line_m: 0.0,
            },
        ],
        crossings: vec![CrossingSource {
            id: "cross".to_owned(),
            region: "area".to_owned(),
            movements: vec!["ew_through".to_owned(), "ns_through".to_owned()],
        }],
        conflict_regions: vec![ConflictRegionSource {
            id: "center".to_owned(),
            points: square("shape", half).points,
            movements: vec!["ew_through".to_owned(), "ns_through".to_owned()],
        }],
        rules: vec![
            RuleSource {
                id: "r_ew".to_owned(),
                movement: "ew_through".to_owned(),
                kind: RuleKind::Signal,
                signal: Some("main".to_owned()),
            },
            RuleSource {
                id: "r_ns".to_owned(),
                movement: "ns_through".to_owned(),
                kind: RuleKind::Signal,
                signal: Some("main".to_owned()),
            },
        ],
        demand: vec![DemandSource {
            id: "inflow".to_owned(),
            portal: "west".to_owned(),
            rate_vph: 600.0,
            routes: vec![RouteShareSource {
                movement: "ew_through".to_owned(),
                weight: 1.0,
            }],
        }],
        profiles: ProfileSource::default(),
        signals: vec![SignalSource {
            id: "main".to_owned(),
            heads: vec![
                SignalHeadSource {
                    id: "ew".to_owned(),
                    movement: "ew_through".to_owned(),
                },
                SignalHeadSource {
                    id: "ns".to_owned(),
                    movement: "ns_through".to_owned(),
                },
            ],
            phases: vec![
                SignalPhaseSource {
                    duration_s: rng.positive(),
                    states: vec![
                        SignalStateSource {
                            head: "ew".to_owned(),
                            color: SignalColor::Green,
                        },
                        SignalStateSource {
                            head: "ns".to_owned(),
                            color: SignalColor::Red,
                        },
                    ],
                },
                SignalPhaseSource {
                    duration_s: rng.positive(),
                    states: vec![
                        SignalStateSource {
                            head: "ew".to_owned(),
                            color: SignalColor::Red,
                        },
                        SignalStateSource {
                            head: "ns".to_owned(),
                            color: SignalColor::Green,
                        },
                    ],
                },
            ],
        }],
        population: PopulationSource::default(),
    }
}
