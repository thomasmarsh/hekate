# increment6_signal_timing_v1 convergence summary

Generated from `convergence_evidence.json` (evidence_version 1, metric_definition_version 3) by `tangle-cli experiment --convergence --summary`. Every number below is that artifact's own.

- Spec: `experiments/increment6_signal_timing_v1/experiment.json` (sha256 `1b16413dad58adc4228c039334c2045a405e59836b34ff012881bbd8eb6383db`)
- Seed bank: `experiments/increment6_signal_timing_v1/seed_bank.json` (sha256 `daeff62dfc8e5db75d23fc700197bd9b203c5719539016799db18a416af6e08f`)
- Seeds: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 (one bank every fidelity ran)
- Fidelities: fast 0.1 s over 3000 steps, standard 0.05 s over 6000 steps, fine 0.02 s over 15000 steps (each covering one simulated duration)
- Tolerance: rule `|fine - coarse| > absolute + relative x |coarse|`; relative 0.05; a countable metric (records, agents) adds an absolute 1; the verdict reads the `standard_to_fine` step, and the relative change is read against the across-seed mean at the coarser fidelity of the step.

## Variants

| Variant | Scenario | Scenario sha256 | Fidelity batches |
| --- | --- | --- | --- |
| `ew_priority` | `scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5` | `3fe1150e29cd91a5447ed5c5e155868cdb77e2a519aa00dc987cb5e8e8a9ca7d` | fast `experiments/increment6_signal_timing_v1/runs/convergence/ew_priority/fast`; standard `experiments/increment6_signal_timing_v1/runs/ew_priority`; fine `experiments/increment6_signal_timing_v1/runs/convergence/ew_priority/fine` |
| `ns_priority` | `scenarios/experiments/four_leg_pedestrian_ns_priority_v1.json5` | `4b79331cf286ae62d1d4ad493ed3c825afd1d9f1b8740bdf52cd35276e2ed22c` | fast `experiments/increment6_signal_timing_v1/runs/convergence/ns_priority/fast`; standard `experiments/increment6_signal_timing_v1/runs/ns_priority`; fine `experiments/increment6_signal_timing_v1/runs/convergence/ns_priority/fine` |

## Selected findings

Method: stable when both variants report an across-seed mean at the reference fidelity and at the judged fidelity and the sign of side_a - side_b is the same at both; flipped when both are nonzero and the signs differ; inconclusive otherwise; the difference is side_a - side_b, of the two variants' across-seed means at one fidelity; judged at `fine`, compared against `fast`. Side A is `ew_priority` and side B is `ns_priority`, the spec's declared order.

| Finding | Family | Slice | Metric | A fast | A standard | A fine | B fast | B standard | B fine | Difference fast | Difference standard | Difference fine | Direction at fine |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Control delay of the east-west through movement | agent_movement | `movement:ew_through` | `mean_control_delay_s` | 13.2767 | 14.1770 | 23.3074 | 23.2344 | 24.4355 | 29.0850 | -9.9577 | -10.2585 | -5.7776 | stable |
| Control delay of the north-south through movement | agent_movement | `movement:ns_through` | `mean_control_delay_s` | 23.3576 | 26.5346 | 35.6561 | 13.1482 | 13.4949 | 25.7165 | 10.2093 | 13.0397 | 9.9396 | stable |
| Control delay over the whole run | run | `*` | `operational.run.mean_control_delay_s` | 13.6671 | 14.7338 | 21.1331 | 13.4660 | 13.8245 | 20.1386 | 0.2012 | 0.9093 | 0.9945 | stable |

## Material sensitivity per metric

Every metric of every slice family the evidence carries, with its across-seed mean at each fidelity, the paired refinement change as a relative change of the coarser fidelity's mean, and the verdict the report reached. `-` is a value the evidence does not carry — not applicable, not observed, or a slice that fidelity does not reach — never a zero.

### ew_priority

#### run slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `*` | `event_counts.by_family.collisions` | records (count) | 7.0000 | 5.9000 | 2.6000 | 15.71% | 55.93% | materially_sensitive |
| `*` | `event_counts.by_family.control_transitions` | records (count) | 220.8000 | 209.0000 | 197.6000 | 5.34% | 5.45% | converged |
| `*` | `event_counts.by_family.despawns` | records (count) | 118.0000 | 110.6000 | 88.1000 | 6.27% | 20.34% | materially_sensitive |
| `*` | `event_counts.by_family.near_misses` | records (count) | 88.3000 | 74.9000 | 106.4000 | 15.18% | 42.06% | materially_sensitive |
| `*` | `event_counts.by_family.queue_events` | records (count) | 425.6000 | 770.6000 | 1849.4000 | 81.06% | 139.99% | materially_sensitive |
| `*` | `event_counts.by_family.region_entries` | records (count) | 195.3000 | 183.9000 | 138.1000 | 5.84% | 24.90% | materially_sensitive |
| `*` | `event_counts.by_family.region_exits` | records (count) | 192.9000 | 181.4000 | 135.9000 | 5.96% | 25.08% | materially_sensitive |
| `*` | `event_counts.by_family.spawns` | records (count) | 134.3000 | 126.8000 | 106.0000 | 5.58% | 16.40% | materially_sensitive |
| `*` | `event_counts.by_family.violations` | records (count) | 8.3000 | 7.7000 | 3.9000 | 7.23% | 49.35% | materially_sensitive |
| `*` | `event_counts.by_family.yields` | records (count) | 140.2000 | 133.4000 | 108.4000 | 4.85% | 18.74% | materially_sensitive |
| `*` | `event_counts.by_family_kind.control_transitions.crossing_wait` | records (count) | 77.4000 | 69.6000 | 70.3000 | 10.08% | 1.01% | converged |
| `*` | `event_counts.by_family_kind.control_transitions.signal_stop` | records (count) | 143.4000 | 139.4000 | 127.3000 | 2.79% | 8.68% | materially_sensitive |
| `*` | `event_counts.by_family_kind.violations.crossed_against_signal` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `*` | `event_counts.by_family_kind.violations.ran_red_light` | records (count) | 8.3000 | 7.7000 | 3.9000 | 7.23% | 49.35% | materially_sensitive |
| `*` | `event_counts.total` | records (count) | 1530.7000 | 1804.2000 | 2736.4000 | 17.87% | 51.67% | materially_sensitive |
| `*` | `minimum_post_encroachment_s` | seconds (continuous) | 0.0100 | 0.0100 | 0.1140 | 0.00% | 1040.00% | materially_sensitive |
| `*` | `minimum_separation_m` | metres (continuous) | -1.8294 | -1.8558 | -1.5817 | 1.44% | 14.77% | materially_sensitive |
| `*` | `minimum_ttc_s` | seconds (continuous) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `*` | `operational.by_mode.pedestrian.maximum_queue_duration_s` | seconds (continuous) | 13.1800 | 16.2700 | 29.6660 | 23.44% | 82.34% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.maximum_queue_length_agents` | agents (count) | 4.1000 | 4.0000 | 6.3000 | 2.44% | 57.50% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_control_delay_s` | seconds (continuous) | 7.6656 | 7.4961 | 14.0206 | 2.21% | 87.04% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_queue_duration_s` | seconds (continuous) | 0.3730 | 0.2619 | 0.4272 | 29.78% | 63.09% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_stopped_delay_s` | seconds (continuous) | 1.7992 | 2.0285 | 7.8722 | 12.75% | 288.08% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_travel_time_s` | seconds (continuous) | 28.7345 | 29.0401 | 35.9721 | 1.06% | 23.87% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.throughput_agents_per_s` | agents_per_second (continuous) | 0.1580 | 0.1467 | 0.1483 | 7.17% | 1.14% | converged |
| `*` | `operational.by_mode.pedestrian.total_control_delay_s` | seconds (continuous) | 363.1400 | 328.1650 | 621.1500 | 9.63% | 89.28% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.total_stopped_delay_s` | seconds (continuous) | 85.1700 | 87.3500 | 351.6880 | 2.56% | 302.62% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.total_travel_time_s` | seconds (continuous) | 1363.6500 | 1276.3900 | 1604.6300 | 6.40% | 25.72% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.maximum_queue_duration_s` | seconds (continuous) | 26.1700 | 26.3000 | 46.2200 | 0.50% | 75.74% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.maximum_queue_length_agents` | agents (count) | 7.4000 | 7.1000 | 10.1000 | 4.05% | 42.25% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_control_delay_s` | seconds (continuous) | 17.7890 | 19.6536 | 28.7576 | 10.48% | 46.32% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_queue_duration_s` | seconds (continuous) | 1.3852 | 0.7920 | 1.0655 | 42.83% | 34.54% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_stopped_delay_s` | seconds (continuous) | 3.6141 | 4.1919 | 16.2376 | 15.99% | 287.36% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_travel_time_s` | seconds (continuous) | 40.1851 | 43.1812 | 60.2160 | 7.46% | 39.45% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.throughput_agents_per_s` | agents_per_second (continuous) | 0.2353 | 0.2220 | 0.1453 | 5.67% | 34.53% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.total_control_delay_s` | seconds (continuous) | 1248.1900 | 1296.6550 | 1221.9740 | 3.88% | 5.76% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.total_stopped_delay_s` | seconds (continuous) | 248.2300 | 278.1800 | 677.7380 | 12.07% | 143.63% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.total_travel_time_s` | seconds (continuous) | 2822.1100 | 2851.3450 | 2559.6720 | 1.04% | 10.23% | materially_sensitive |
| `*` | `operational.run.maximum_queue_duration_s` | seconds (continuous) | 26.1700 | 26.3000 | 46.2200 | 0.50% | 75.74% | materially_sensitive |
| `*` | `operational.run.maximum_queue_length_agents` | agents (count) | 9.6000 | 8.6000 | 14.6000 | 10.42% | 69.77% | materially_sensitive |
| `*` | `operational.run.mean_control_delay_s` | seconds (continuous) | 13.6671 | 14.7338 | 21.1331 | 7.80% | 43.43% | materially_sensitive |
| `*` | `operational.run.mean_queue_duration_s` | seconds (continuous) | 0.8107 | 0.5287 | 0.6935 | 34.79% | 31.19% | materially_sensitive |
| `*` | `operational.run.mean_stopped_delay_s` | seconds (continuous) | 2.8462 | 3.3133 | 11.7851 | 16.41% | 255.69% | materially_sensitive |
| `*` | `operational.run.mean_travel_time_s` | seconds (continuous) | 35.5004 | 37.3997 | 47.6058 | 5.35% | 27.29% | materially_sensitive |
| `*` | `operational.run.throughput_agents_per_s` | agents_per_second (continuous) | 0.3933 | 0.3687 | 0.2937 | 6.27% | 20.34% | materially_sensitive |
| `*` | `operational.run.total_control_delay_s` | seconds (continuous) | 1611.3300 | 1624.8200 | 1843.1240 | 0.84% | 13.44% | materially_sensitive |
| `*` | `operational.run.total_stopped_delay_s` | seconds (continuous) | 333.4000 | 365.5300 | 1029.4260 | 9.64% | 181.63% | materially_sensitive |
| `*` | `operational.run.total_travel_time_s` | seconds (continuous) | 4185.7600 | 4127.7350 | 4164.3020 | 1.39% | 0.89% | converged |

#### mode slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pedestrian` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian` | `event_counts.control_transitions` | records (count) | 77.4000 | 69.6000 | 70.3000 | 10.08% | 1.01% | converged |
| `pedestrian` | `event_counts.despawns` | records (count) | 47.4000 | 44.0000 | 44.5000 | 7.17% | 1.14% | converged |
| `pedestrian` | `event_counts.near_misses` | records (count) | 71.0000 | 59.7000 | 98.0000 | 15.92% | 64.15% | materially_sensitive |
| `pedestrian` | `event_counts.queue_events` | records (count) | 232.7000 | 370.3000 | 971.9000 | 59.13% | 162.46% | materially_sensitive |
| `pedestrian` | `event_counts.region_entries` | records (count) | 50.3000 | 46.8000 | 48.8000 | 6.96% | 4.27% | converged |
| `pedestrian` | `event_counts.region_exits` | records (count) | 48.9000 | 45.4000 | 47.1000 | 7.16% | 3.74% | converged |
| `pedestrian` | `event_counts.spawns` | records (count) | 51.6000 | 48.2000 | 50.2000 | 6.59% | 4.15% | converged |
| `pedestrian` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `vehicle` | `event_counts.collisions` | records (count) | 7.0000 | 5.9000 | 2.6000 | 15.71% | 55.93% | materially_sensitive |
| `vehicle` | `event_counts.control_transitions` | records (count) | 143.4000 | 139.4000 | 127.3000 | 2.79% | 8.68% | materially_sensitive |
| `vehicle` | `event_counts.despawns` | records (count) | 70.6000 | 66.6000 | 43.6000 | 5.67% | 34.53% | materially_sensitive |
| `vehicle` | `event_counts.near_misses` | records (count) | 17.3000 | 15.2000 | 8.4000 | 12.14% | 44.74% | materially_sensitive |
| `vehicle` | `event_counts.queue_events` | records (count) | 192.9000 | 400.3000 | 877.5000 | 107.52% | 119.21% | materially_sensitive |
| `vehicle` | `event_counts.region_entries` | records (count) | 145.0000 | 137.1000 | 89.3000 | 5.45% | 34.87% | materially_sensitive |
| `vehicle` | `event_counts.region_exits` | records (count) | 144.0000 | 136.0000 | 88.8000 | 5.56% | 34.71% | materially_sensitive |
| `vehicle` | `event_counts.spawns` | records (count) | 82.7000 | 78.6000 | 55.8000 | 4.96% | 29.01% | materially_sensitive |
| `vehicle` | `event_counts.violations` | records (count) | 8.3000 | 7.7000 | 3.9000 | 7.23% | 49.35% | materially_sensitive |
| `vehicle` | `event_counts.yields` | records (count) | 140.2000 | 133.4000 | 108.4000 | 4.85% | 18.74% | materially_sensitive |

#### mode_pair slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pedestrian_pedestrian` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `vehicle_pedestrian` | `minimum_separation_m` | metres (continuous) | 0.7360 | 0.8685 | 0.6159 | 18.00% | 29.08% | materially_sensitive |
| `vehicle_vehicle` | `minimum_separation_m` | metres (continuous) | -1.8294 | -1.8558 | -1.5817 | 1.44% | 14.77% | materially_sensitive |

#### movement_pair slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `movement:ew_through|movement:ew_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.0500 | 0.0400 | 0.2960 | 20.00% | 640.00% | materially_sensitive |
| `movement:ew_through|movement:ew_through` | `minimum_separation_m` | metres (continuous) | 0.7404 | 0.8801 | 0.9021 | 18.88% | 2.49% | converged |
| `movement:ew_through|movement:ew_through` | `minimum_ttc_s` | seconds (continuous) | 1.1543 | 1.2265 | 1.1630 | 6.25% | 5.17% | materially_sensitive |
| `movement:ew_through|movement:ns_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.0700 | 0.1700 | 0.5040 | 142.86% | 196.47% | materially_sensitive |
| `movement:ew_through|movement:ns_through` | `minimum_separation_m` | metres (continuous) | -1.8294 | -1.8558 | -1.5817 | 1.44% | 14.77% | materially_sensitive |
| `movement:ew_through|movement:ns_through` | `minimum_ttc_s` | seconds (continuous) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 6.6821 | 6.7287 | 6.6476 | 0.70% | 1.21% | converged |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 5.9599 | 6.3168 | 6.1675 | 5.99% | 2.36% | converged |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | 0.2700 | 0.3150 | 0.5060 | 16.67% | 60.63% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 1.3071 | 1.5079 | 1.4918 | 15.36% | 1.07% | converged |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | 1.7180 | 1.5155 | 1.3888 | 11.79% | 8.36% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | 0.2700 | 0.3200 | 0.5460 | 18.52% | 70.62% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 1.0603 | 1.0765 | 0.6775 | 1.53% | 37.07% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 1.5922 | 1.6628 | 1.5391 | 4.43% | 7.44% | materially_sensitive |
| `movement:ns_through|movement:ns_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.0800 | 0.1550 | 0.3660 | 93.75% | 136.13% | materially_sensitive |
| `movement:ns_through|movement:ns_through` | `minimum_separation_m` | metres (continuous) | 0.7293 | 0.7296 | 0.9231 | 0.05% | 26.52% | materially_sensitive |
| `movement:ns_through|movement:ns_through` | `minimum_ttc_s` | seconds (continuous) | 1.1162 | 1.0800 | 1.3142 | 3.24% | 21.68% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | 0.4000 | 0.3700 | 0.4460 | 7.50% | 20.54% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 1.1352 | 1.2947 | 0.9222 | 14.05% | 28.77% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | 1.5968 | 2.0119 | 1.9644 | 26.00% | 2.36% | converged |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 0.3900 | 0.3350 | 0.4844 | 14.10% | 48.42% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 1.5043 | 1.4151 | 1.4336 | 5.93% | 1.31% | converged |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 1.8391 | 2.0936 | 1.7172 | 13.84% | 17.98% | materially_sensitive |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 5.7958 | 6.1867 | 5.8795 | 6.74% | 4.97% | converged |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 6.6914 | 6.6872 | 6.7110 | 0.06% | 0.35% | converged |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | 2.0500 | 1.5375 | - | 158.54% | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 0.1714 | 1.1643 | 0.1406 | 579.48% | 87.92% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | 0.3736 | 0.1096 | 0.3955 | 80.50% | 292.91% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 12.1000 | 7.8000 | 3.4900 | - | 126.67% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 0.0511 | 0.0502 | 0.0355 | 1.83% | 29.28% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 3.1232 | 3.1134 | 3.0883 | 0.32% | 0.81% | converged |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 3.3796 | 4.4004 | 3.9309 | 30.20% | 10.67% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 1.5333 | 2.7000 | 3.8950 | 39.13% | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 0.0560 | 1.6926 | 0.8577 | 2925.01% | 60.08% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 0.0753 | 1.3209 | 0.9019 | 1652.05% | 67.58% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 4.6768 | 4.3838 | 4.1286 | 6.27% | 5.82% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 3.3217 | 3.4603 | 4.7230 | 4.17% | 36.49% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 2.7053 | 1.9751 | 3.0556 | 19.39% | 54.84% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | 0.4333 | 1.9000 | 1.1000 | 38.46% | 29.47% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 0.2565 | 0.1510 | 0.0500 | 41.10% | 66.90% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | 0.0837 | 0.3897 | 0.0549 | 258.34% | 85.90% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | 4.7000 | 5.2900 | 4.8560 | 169.68% | 73.16% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 0.0483 | 0.0470 | 0.0360 | 2.72% | 23.47% | materially_sensitive |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | 0.2000 | 1.8500 | 17.8800 | - | - | inconclusive |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 0.5008 | 0.0581 | 0.0500 | 88.39% | 14.01% | materially_sensitive |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 0.2340 | 0.0932 | 0.0620 | 59.38% | 33.45% | materially_sensitive |

#### agent_movement slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `movement:ew_through` | `event_counts.collisions` | records (count) | 3.2000 | 2.0000 | 1.5000 | 37.50% | 25.00% | converged |
| `movement:ew_through` | `event_counts.control_transitions` | records (count) | 67.2000 | 62.6000 | 57.4000 | 6.85% | 8.31% | materially_sensitive |
| `movement:ew_through` | `event_counts.despawns` | records (count) | 38.5000 | 35.7000 | 21.8000 | 7.27% | 38.94% | materially_sensitive |
| `movement:ew_through` | `event_counts.near_misses` | records (count) | 8.0000 | 5.7000 | 4.8000 | 28.75% | 15.79% | converged |
| `movement:ew_through` | `event_counts.queue_events` | records (count) | 88.1000 | 165.2000 | 362.8000 | 87.51% | 119.61% | materially_sensitive |
| `movement:ew_through` | `event_counts.region_entries` | records (count) | 79.8000 | 74.2000 | 44.2000 | 7.02% | 40.43% | materially_sensitive |
| `movement:ew_through` | `event_counts.region_exits` | records (count) | 79.1000 | 73.4000 | 44.1000 | 7.21% | 39.92% | materially_sensitive |
| `movement:ew_through` | `event_counts.spawns` | records (count) | 44.8000 | 41.7000 | 27.9000 | 6.92% | 33.09% | materially_sensitive |
| `movement:ew_through` | `event_counts.violations` | records (count) | 3.3000 | 3.1000 | 1.2000 | 6.06% | 61.29% | materially_sensitive |
| `movement:ew_through` | `event_counts.yields` | records (count) | 71.9000 | 66.5000 | 51.9000 | 7.51% | 21.95% | materially_sensitive |
| `movement:ew_through` | `maximum_queue_duration_s` | seconds (continuous) | 24.2800 | 22.5950 | 40.5620 | 6.94% | 79.52% | materially_sensitive |
| `movement:ew_through` | `maximum_queue_length_agents` | agents (count) | 5.0000 | 4.8000 | 6.5000 | 4.00% | 35.42% | materially_sensitive |
| `movement:ew_through` | `mean_control_delay_s` | seconds (continuous) | 13.2767 | 14.1770 | 23.3074 | 6.78% | 64.40% | materially_sensitive |
| `movement:ew_through` | `mean_queue_duration_s` | seconds (continuous) | 1.6803 | 1.1732 | 1.5330 | 30.18% | 30.67% | materially_sensitive |
| `movement:ew_through` | `mean_stopped_delay_s` | seconds (continuous) | 3.6626 | 4.4192 | 20.5157 | 20.66% | 364.24% | materially_sensitive |
| `movement:ew_through` | `mean_travel_time_s` | seconds (continuous) | 36.4564 | 37.8568 | 58.6329 | 3.84% | 54.88% | materially_sensitive |
| `movement:ew_through` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.1283 | 0.1190 | 0.0727 | 7.27% | 38.94% | materially_sensitive |
| `movement:ew_through` | `total_control_delay_s` | seconds (continuous) | 507.8300 | 503.4850 | 479.8000 | 0.86% | 4.70% | converged |
| `movement:ew_through` | `total_stopped_delay_s` | seconds (continuous) | 137.1200 | 155.6250 | 409.0640 | 13.50% | 162.85% | materially_sensitive |
| `movement:ew_through` | `total_travel_time_s` | seconds (continuous) | 1395.4000 | 1342.7250 | 1206.2420 | 3.77% | 10.16% | materially_sensitive |
| `movement:ns_through` | `event_counts.collisions` | records (count) | 3.8000 | 3.9000 | 1.1000 | 2.63% | 71.79% | materially_sensitive |
| `movement:ns_through` | `event_counts.control_transitions` | records (count) | 76.2000 | 76.8000 | 69.9000 | 0.79% | 8.98% | materially_sensitive |
| `movement:ns_through` | `event_counts.despawns` | records (count) | 32.1000 | 30.9000 | 21.8000 | 3.74% | 29.45% | materially_sensitive |
| `movement:ns_through` | `event_counts.near_misses` | records (count) | 9.3000 | 9.5000 | 3.6000 | 2.15% | 62.11% | materially_sensitive |
| `movement:ns_through` | `event_counts.queue_events` | records (count) | 104.8000 | 235.1000 | 514.7000 | 124.33% | 118.93% | materially_sensitive |
| `movement:ns_through` | `event_counts.region_entries` | records (count) | 65.2000 | 62.9000 | 45.1000 | 3.53% | 28.30% | materially_sensitive |
| `movement:ns_through` | `event_counts.region_exits` | records (count) | 64.9000 | 62.6000 | 44.7000 | 3.54% | 28.59% | materially_sensitive |
| `movement:ns_through` | `event_counts.spawns` | records (count) | 37.9000 | 36.9000 | 27.9000 | 2.64% | 24.39% | materially_sensitive |
| `movement:ns_through` | `event_counts.violations` | records (count) | 5.0000 | 4.6000 | 2.7000 | 8.00% | 41.30% | materially_sensitive |
| `movement:ns_through` | `event_counts.yields` | records (count) | 68.3000 | 66.9000 | 56.5000 | 2.05% | 15.55% | materially_sensitive |
| `movement:ns_through` | `maximum_queue_duration_s` | seconds (continuous) | 15.8200 | 17.8750 | 33.0240 | 12.99% | 84.75% | materially_sensitive |
| `movement:ns_through` | `maximum_queue_length_agents` | agents (count) | 4.7000 | 5.2000 | 6.3000 | 10.64% | 21.15% | converged |
| `movement:ns_through` | `mean_control_delay_s` | seconds (continuous) | 23.3576 | 26.5346 | 35.6561 | 13.60% | 34.38% | materially_sensitive |
| `movement:ns_through` | `mean_queue_duration_s` | seconds (continuous) | 1.1797 | 0.5432 | 0.7745 | 53.95% | 42.58% | materially_sensitive |
| `movement:ns_through` | `mean_stopped_delay_s` | seconds (continuous) | 3.6687 | 4.3648 | 13.8866 | 18.98% | 218.15% | materially_sensitive |
| `movement:ns_through` | `mean_travel_time_s` | seconds (continuous) | 44.9760 | 50.3850 | 64.8092 | 12.03% | 28.63% | materially_sensitive |
| `movement:ns_through` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.1070 | 0.1030 | 0.0727 | 3.74% | 29.45% | materially_sensitive |
| `movement:ns_through` | `total_control_delay_s` | seconds (continuous) | 740.3600 | 793.1700 | 742.1740 | 7.13% | 6.43% | materially_sensitive |
| `movement:ns_through` | `total_stopped_delay_s` | seconds (continuous) | 111.1100 | 122.5550 | 268.6740 | 10.30% | 119.23% | materially_sensitive |
| `movement:ns_through` | `total_travel_time_s` | seconds (continuous) | 1426.7100 | 1508.6200 | 1353.4300 | 5.74% | 10.29% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `event_counts.control_transitions` | records (count) | 13.4000 | 12.6000 | 14.6000 | 5.97% | 15.87% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.despawns` | records (count) | 11.8000 | 10.8000 | 11.2000 | 8.47% | 3.70% | converged |
| `pedestrian_route:south_to_east` | `event_counts.near_misses` | records (count) | 16.5000 | 13.8000 | 22.7000 | 16.36% | 64.49% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.queue_events` | records (count) | 69.3000 | 90.1000 | 236.3000 | 30.01% | 162.26% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.region_entries` | records (count) | 13.1000 | 11.6000 | 12.6000 | 11.45% | 8.62% | converged |
| `pedestrian_route:south_to_east` | `event_counts.region_exits` | records (count) | 12.5000 | 11.1000 | 12.5000 | 11.20% | 12.61% | converged |
| `pedestrian_route:south_to_east` | `event_counts.spawns` | records (count) | 13.3000 | 11.7000 | 12.7000 | 12.03% | 8.55% | converged |
| `pedestrian_route:south_to_east` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `maximum_queue_duration_s` | seconds (continuous) | 4.3900 | 4.7500 | 19.7260 | 8.20% | 315.28% | materially_sensitive |
| `pedestrian_route:south_to_east` | `maximum_queue_length_agents` | agents (count) | 1.9000 | 1.8000 | 2.7000 | 5.26% | 50.00% | converged |
| `pedestrian_route:south_to_east` | `mean_control_delay_s` | seconds (continuous) | 2.1494 | 2.8028 | 7.1719 | 30.39% | 155.89% | materially_sensitive |
| `pedestrian_route:south_to_east` | `mean_queue_duration_s` | seconds (continuous) | 0.2772 | 0.2228 | 0.4277 | 19.62% | 91.95% | materially_sensitive |
| `pedestrian_route:south_to_east` | `mean_stopped_delay_s` | seconds (continuous) | 1.4963 | 1.8552 | 5.9412 | 23.99% | 220.25% | materially_sensitive |
| `pedestrian_route:south_to_east` | `mean_travel_time_s` | seconds (continuous) | 28.5136 | 29.0398 | 33.6951 | 1.85% | 16.03% | materially_sensitive |
| `pedestrian_route:south_to_east` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0393 | 0.0360 | 0.0373 | 8.47% | 3.70% | converged |
| `pedestrian_route:south_to_east` | `total_control_delay_s` | seconds (continuous) | 25.2700 | 30.0350 | 81.8820 | 18.86% | 172.62% | materially_sensitive |
| `pedestrian_route:south_to_east` | `total_stopped_delay_s` | seconds (continuous) | 17.6900 | 20.4900 | 67.7140 | 15.83% | 230.47% | materially_sensitive |
| `pedestrian_route:south_to_east` | `total_travel_time_s` | seconds (continuous) | 336.4100 | 313.7300 | 379.6420 | 6.74% | 21.01% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `event_counts.control_transitions` | records (count) | 20.6000 | 17.4000 | 16.2000 | 15.53% | 6.90% | converged |
| `pedestrian_route:south_to_west` | `event_counts.despawns` | records (count) | 12.5000 | 10.1000 | 10.2000 | 19.20% | 0.99% | converged |
| `pedestrian_route:south_to_west` | `event_counts.near_misses` | records (count) | 21.3000 | 12.8000 | 19.2000 | 39.91% | 50.00% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.queue_events` | records (count) | 66.2000 | 89.5000 | 241.2000 | 35.20% | 169.50% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.region_entries` | records (count) | 12.9000 | 10.5000 | 10.4000 | 18.60% | 0.95% | converged |
| `pedestrian_route:south_to_west` | `event_counts.region_exits` | records (count) | 12.6000 | 10.1000 | 10.3000 | 19.84% | 1.98% | converged |
| `pedestrian_route:south_to_west` | `event_counts.spawns` | records (count) | 13.4000 | 11.1000 | 10.9000 | 17.16% | 1.80% | converged |
| `pedestrian_route:south_to_west` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `maximum_queue_duration_s` | seconds (continuous) | 2.5100 | 5.9300 | 15.3340 | 136.25% | 158.58% | materially_sensitive |
| `pedestrian_route:south_to_west` | `maximum_queue_length_agents` | agents (count) | 2.0000 | 1.5000 | 2.0000 | 25.00% | 33.33% | converged |
| `pedestrian_route:south_to_west` | `mean_control_delay_s` | seconds (continuous) | 10.3238 | 9.1041 | 14.4149 | 11.81% | 58.34% | materially_sensitive |
| `pedestrian_route:south_to_west` | `mean_queue_duration_s` | seconds (continuous) | 0.2540 | 0.2483 | 0.3069 | 2.24% | 23.57% | materially_sensitive |
| `pedestrian_route:south_to_west` | `mean_stopped_delay_s` | seconds (continuous) | 1.4335 | 2.0206 | 6.2375 | 40.95% | 208.69% | materially_sensitive |
| `pedestrian_route:south_to_west` | `mean_travel_time_s` | seconds (continuous) | 28.9298 | 29.2018 | 34.4370 | 0.94% | 17.93% | materially_sensitive |
| `pedestrian_route:south_to_west` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0417 | 0.0337 | 0.0340 | 19.20% | 0.99% | converged |
| `pedestrian_route:south_to_west` | `total_control_delay_s` | seconds (continuous) | 131.0500 | 92.3350 | 143.5880 | 29.54% | 55.51% | materially_sensitive |
| `pedestrian_route:south_to_west` | `total_stopped_delay_s` | seconds (continuous) | 17.4000 | 20.1500 | 63.6100 | 15.80% | 215.68% | materially_sensitive |
| `pedestrian_route:south_to_west` | `total_travel_time_s` | seconds (continuous) | 361.3500 | 293.3850 | 350.0140 | 18.81% | 19.30% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `event_counts.control_transitions` | records (count) | 17.8000 | 16.0000 | 16.7000 | 10.11% | 4.38% | converged |
| `pedestrian_route:west_to_north` | `event_counts.despawns` | records (count) | 10.8000 | 11.8000 | 12.4000 | 9.26% | 5.08% | converged |
| `pedestrian_route:west_to_north` | `event_counts.near_misses` | records (count) | 17.1000 | 16.0000 | 28.5000 | 6.43% | 78.12% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.queue_events` | records (count) | 49.7000 | 93.8000 | 234.0000 | 88.73% | 149.47% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.region_entries` | records (count) | 11.7000 | 13.2000 | 14.6000 | 12.82% | 10.61% | converged |
| `pedestrian_route:west_to_north` | `event_counts.region_exits` | records (count) | 11.5000 | 12.9000 | 13.6000 | 12.17% | 5.43% | converged |
| `pedestrian_route:west_to_north` | `event_counts.spawns` | records (count) | 11.6000 | 13.2000 | 14.7000 | 13.79% | 11.36% | converged |
| `pedestrian_route:west_to_north` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `maximum_queue_duration_s` | seconds (continuous) | 10.2600 | 8.4850 | 28.5380 | 17.30% | 236.33% | materially_sensitive |
| `pedestrian_route:west_to_north` | `maximum_queue_length_agents` | agents (count) | 2.0000 | 1.8000 | 2.8000 | 10.00% | 55.56% | converged |
| `pedestrian_route:west_to_north` | `mean_control_delay_s` | seconds (continuous) | 4.9622 | 3.4980 | 12.0476 | 29.51% | 244.42% | materially_sensitive |
| `pedestrian_route:west_to_north` | `mean_queue_duration_s` | seconds (continuous) | 0.6046 | 0.2839 | 0.5884 | 53.03% | 107.23% | materially_sensitive |
| `pedestrian_route:west_to_north` | `mean_stopped_delay_s` | seconds (continuous) | 2.6688 | 2.0196 | 9.8028 | 24.32% | 385.38% | materially_sensitive |
| `pedestrian_route:west_to_north` | `mean_travel_time_s` | seconds (continuous) | 29.7657 | 28.9128 | 37.7317 | 2.87% | 30.50% | materially_sensitive |
| `pedestrian_route:west_to_north` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0360 | 0.0393 | 0.0413 | 9.26% | 5.08% | materially_sensitive |
| `pedestrian_route:west_to_north` | `total_control_delay_s` | seconds (continuous) | 51.0700 | 39.5150 | 146.9400 | 22.63% | 271.86% | materially_sensitive |
| `pedestrian_route:west_to_north` | `total_stopped_delay_s` | seconds (continuous) | 27.8600 | 22.5300 | 119.2780 | 19.13% | 429.42% | materially_sensitive |
| `pedestrian_route:west_to_north` | `total_travel_time_s` | seconds (continuous) | 320.9700 | 338.8250 | 466.3360 | 5.56% | 37.63% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `event_counts.control_transitions` | records (count) | 25.6000 | 23.6000 | 22.8000 | 7.81% | 3.39% | converged |
| `pedestrian_route:west_to_south` | `event_counts.despawns` | records (count) | 12.3000 | 11.3000 | 10.7000 | 8.13% | 5.31% | converged |
| `pedestrian_route:west_to_south` | `event_counts.near_misses` | records (count) | 16.1000 | 17.1000 | 27.6000 | 6.21% | 61.40% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.queue_events` | records (count) | 47.5000 | 96.9000 | 260.4000 | 104.00% | 168.73% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.region_entries` | records (count) | 12.6000 | 11.5000 | 11.2000 | 8.73% | 2.61% | converged |
| `pedestrian_route:west_to_south` | `event_counts.region_exits` | records (count) | 12.3000 | 11.3000 | 10.7000 | 8.13% | 5.31% | converged |
| `pedestrian_route:west_to_south` | `event_counts.spawns` | records (count) | 13.3000 | 12.2000 | 11.9000 | 8.27% | 2.46% | converged |
| `pedestrian_route:west_to_south` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `maximum_queue_duration_s` | seconds (continuous) | 7.3900 | 10.0400 | 25.5840 | 35.86% | 154.82% | materially_sensitive |
| `pedestrian_route:west_to_south` | `maximum_queue_length_agents` | agents (count) | 1.6000 | 2.1000 | 3.2000 | 31.25% | 52.38% | converged |
| `pedestrian_route:west_to_south` | `mean_control_delay_s` | seconds (continuous) | 12.7633 | 15.4771 | 23.3105 | 21.26% | 50.61% | materially_sensitive |
| `pedestrian_route:west_to_south` | `mean_queue_duration_s` | seconds (continuous) | 0.5948 | 0.2929 | 0.4635 | 50.75% | 58.24% | materially_sensitive |
| `pedestrian_route:west_to_south` | `mean_stopped_delay_s` | seconds (continuous) | 1.9920 | 2.4142 | 9.4899 | 21.20% | 293.09% | materially_sensitive |
| `pedestrian_route:west_to_south` | `mean_travel_time_s` | seconds (continuous) | 28.1836 | 29.5173 | 38.1285 | 4.73% | 29.17% | materially_sensitive |
| `pedestrian_route:west_to_south` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0410 | 0.0377 | 0.0357 | 8.13% | 5.31% | materially_sensitive |
| `pedestrian_route:west_to_south` | `total_control_delay_s` | seconds (continuous) | 155.7500 | 166.2800 | 248.7400 | 6.76% | 49.59% | materially_sensitive |
| `pedestrian_route:west_to_south` | `total_stopped_delay_s` | seconds (continuous) | 22.2200 | 24.1800 | 101.0860 | 8.82% | 318.06% | materially_sensitive |
| `pedestrian_route:west_to_south` | `total_travel_time_s` | seconds (continuous) | 344.9200 | 330.4500 | 408.6380 | 4.20% | 23.66% | materially_sensitive |

### ns_priority

#### run slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `*` | `event_counts.by_family.collisions` | records (count) | 6.5000 | 6.1000 | 2.9000 | 6.15% | 52.46% | materially_sensitive |
| `*` | `event_counts.by_family.control_transitions` | records (count) | 215.0000 | 203.3000 | 196.4000 | 5.44% | 3.39% | converged |
| `*` | `event_counts.by_family.despawns` | records (count) | 116.2000 | 112.6000 | 90.3000 | 3.10% | 19.80% | materially_sensitive |
| `*` | `event_counts.by_family.near_misses` | records (count) | 83.7000 | 77.6000 | 108.4000 | 7.29% | 39.69% | materially_sensitive |
| `*` | `event_counts.by_family.queue_events` | records (count) | 445.1000 | 777.8000 | 1695.2000 | 74.75% | 117.95% | materially_sensitive |
| `*` | `event_counts.by_family.region_entries` | records (count) | 191.6000 | 187.4000 | 142.7000 | 2.19% | 23.85% | materially_sensitive |
| `*` | `event_counts.by_family.region_exits` | records (count) | 189.5000 | 184.9000 | 140.4000 | 2.43% | 24.07% | materially_sensitive |
| `*` | `event_counts.by_family.spawns` | records (count) | 132.6000 | 127.5000 | 108.2000 | 3.85% | 15.14% | materially_sensitive |
| `*` | `event_counts.by_family.violations` | records (count) | 8.9000 | 8.7000 | 4.6000 | 2.25% | 47.13% | materially_sensitive |
| `*` | `event_counts.by_family.yields` | records (count) | 133.3000 | 131.9000 | 104.3000 | 1.05% | 20.92% | materially_sensitive |
| `*` | `event_counts.by_family_kind.control_transitions.crossing_wait` | records (count) | 78.4000 | 69.0000 | 70.5000 | 11.99% | 2.17% | converged |
| `*` | `event_counts.by_family_kind.control_transitions.signal_stop` | records (count) | 136.6000 | 134.3000 | 125.9000 | 1.68% | 6.25% | materially_sensitive |
| `*` | `event_counts.by_family_kind.violations.crossed_against_signal` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `*` | `event_counts.by_family_kind.violations.ran_red_light` | records (count) | 8.9000 | 8.7000 | 4.6000 | 2.25% | 47.13% | materially_sensitive |
| `*` | `event_counts.total` | records (count) | 1522.4000 | 1817.8000 | 2593.4000 | 19.40% | 42.67% | materially_sensitive |
| `*` | `minimum_post_encroachment_s` | seconds (continuous) | 0.0000 | 0.0200 | 0.0480 | - | 140.00% | materially_sensitive |
| `*` | `minimum_separation_m` | metres (continuous) | -1.7368 | -1.8337 | -1.5067 | 5.58% | 17.83% | materially_sensitive |
| `*` | `minimum_ttc_s` | seconds (continuous) | 0.0000 | 0.0000 | 0.0033 | - | - | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.maximum_queue_duration_s` | seconds (continuous) | 12.3800 | 14.2100 | 30.7720 | 14.78% | 116.55% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.maximum_queue_length_agents` | agents (count) | 4.3000 | 4.4000 | 6.6000 | 2.33% | 50.00% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_control_delay_s` | seconds (continuous) | 7.3030 | 7.2832 | 13.9818 | 0.27% | 91.97% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_queue_duration_s` | seconds (continuous) | 0.3415 | 0.2264 | 0.4753 | 33.71% | 109.97% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_stopped_delay_s` | seconds (continuous) | 1.4956 | 1.8894 | 7.6866 | 26.33% | 306.82% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_travel_time_s` | seconds (continuous) | 28.3719 | 29.1247 | 35.6923 | 2.65% | 22.55% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.throughput_agents_per_s` | agents_per_second (continuous) | 0.1567 | 0.1477 | 0.1483 | 5.74% | 0.45% | converged |
| `*` | `operational.by_mode.pedestrian.total_control_delay_s` | seconds (continuous) | 344.3900 | 324.7050 | 622.9600 | 5.72% | 91.85% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.total_stopped_delay_s` | seconds (continuous) | 70.7900 | 85.0400 | 346.5140 | 20.13% | 307.47% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.total_travel_time_s` | seconds (continuous) | 1335.3600 | 1292.2350 | 1597.1160 | 3.23% | 23.59% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.maximum_queue_duration_s` | seconds (continuous) | 27.1200 | 23.9650 | 39.8940 | 11.63% | 66.47% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.maximum_queue_length_agents` | agents (count) | 7.2000 | 7.1000 | 9.8000 | 1.39% | 38.03% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_control_delay_s` | seconds (continuous) | 17.7382 | 18.0510 | 26.8075 | 1.76% | 48.51% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_queue_duration_s` | seconds (continuous) | 1.4018 | 0.6785 | 1.0832 | 51.60% | 59.66% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_stopped_delay_s` | seconds (continuous) | 3.5780 | 3.6986 | 15.4293 | 3.37% | 317.17% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_travel_time_s` | seconds (continuous) | 39.8645 | 40.5374 | 57.2706 | 1.69% | 41.28% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.throughput_agents_per_s` | agents_per_second (continuous) | 0.2307 | 0.2277 | 0.1527 | 1.30% | 32.94% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.total_control_delay_s` | seconds (continuous) | 1218.2600 | 1231.1800 | 1174.3700 | 1.06% | 4.61% | converged |
| `*` | `operational.by_mode.vehicle.total_stopped_delay_s` | seconds (continuous) | 241.5200 | 249.4000 | 648.7260 | 3.26% | 160.11% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.total_travel_time_s` | seconds (continuous) | 2740.7000 | 2764.2450 | 2517.2740 | 0.86% | 8.93% | materially_sensitive |
| `*` | `operational.run.maximum_queue_duration_s` | seconds (continuous) | 27.1200 | 23.9650 | 39.8940 | 11.63% | 66.47% | materially_sensitive |
| `*` | `operational.run.maximum_queue_length_agents` | agents (count) | 9.2000 | 9.8000 | 14.4000 | 6.52% | 46.94% | materially_sensitive |
| `*` | `operational.run.mean_control_delay_s` | seconds (continuous) | 13.4660 | 13.8245 | 20.1386 | 2.66% | 45.67% | materially_sensitive |
| `*` | `operational.run.mean_queue_duration_s` | seconds (continuous) | 0.8372 | 0.4507 | 0.7641 | 46.17% | 69.56% | materially_sensitive |
| `*` | `operational.run.mean_stopped_delay_s` | seconds (continuous) | 2.6926 | 2.9644 | 11.2750 | 10.10% | 280.34% | materially_sensitive |
| `*` | `operational.run.mean_travel_time_s` | seconds (continuous) | 35.1062 | 36.0368 | 45.9804 | 2.65% | 27.59% | materially_sensitive |
| `*` | `operational.run.throughput_agents_per_s` | agents_per_second (continuous) | 0.3873 | 0.3753 | 0.3010 | 3.10% | 19.80% | materially_sensitive |
| `*` | `operational.run.total_control_delay_s` | seconds (continuous) | 1562.6500 | 1555.8850 | 1797.3300 | 0.43% | 15.52% | materially_sensitive |
| `*` | `operational.run.total_stopped_delay_s` | seconds (continuous) | 312.3100 | 334.4400 | 995.2400 | 7.09% | 197.58% | materially_sensitive |
| `*` | `operational.run.total_travel_time_s` | seconds (continuous) | 4076.0600 | 4056.4800 | 4114.3900 | 0.48% | 1.43% | converged |

#### mode slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pedestrian` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian` | `event_counts.control_transitions` | records (count) | 78.4000 | 69.0000 | 70.5000 | 11.99% | 2.17% | converged |
| `pedestrian` | `event_counts.despawns` | records (count) | 47.0000 | 44.3000 | 44.5000 | 5.74% | 0.45% | converged |
| `pedestrian` | `event_counts.near_misses` | records (count) | 67.3000 | 63.2000 | 98.8000 | 6.09% | 56.33% | materially_sensitive |
| `pedestrian` | `event_counts.queue_events` | records (count) | 235.4000 | 384.7000 | 852.5000 | 63.42% | 121.60% | materially_sensitive |
| `pedestrian` | `event_counts.region_entries` | records (count) | 50.1000 | 46.9000 | 48.9000 | 6.39% | 4.26% | converged |
| `pedestrian` | `event_counts.region_exits` | records (count) | 49.0000 | 45.7000 | 47.2000 | 6.73% | 3.28% | converged |
| `pedestrian` | `event_counts.spawns` | records (count) | 51.6000 | 48.2000 | 50.2000 | 6.59% | 4.15% | converged |
| `pedestrian` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `vehicle` | `event_counts.collisions` | records (count) | 6.5000 | 6.1000 | 2.9000 | 6.15% | 52.46% | materially_sensitive |
| `vehicle` | `event_counts.control_transitions` | records (count) | 136.6000 | 134.3000 | 125.9000 | 1.68% | 6.25% | materially_sensitive |
| `vehicle` | `event_counts.despawns` | records (count) | 69.2000 | 68.3000 | 45.8000 | 1.30% | 32.94% | materially_sensitive |
| `vehicle` | `event_counts.near_misses` | records (count) | 16.4000 | 14.4000 | 9.6000 | 12.20% | 33.33% | materially_sensitive |
| `vehicle` | `event_counts.queue_events` | records (count) | 209.7000 | 393.1000 | 842.7000 | 87.46% | 114.37% | materially_sensitive |
| `vehicle` | `event_counts.region_entries` | records (count) | 141.5000 | 140.5000 | 93.8000 | 0.71% | 33.24% | materially_sensitive |
| `vehicle` | `event_counts.region_exits` | records (count) | 140.5000 | 139.2000 | 93.2000 | 0.93% | 33.05% | materially_sensitive |
| `vehicle` | `event_counts.spawns` | records (count) | 81.0000 | 79.3000 | 58.0000 | 2.10% | 26.86% | materially_sensitive |
| `vehicle` | `event_counts.violations` | records (count) | 8.9000 | 8.7000 | 4.6000 | 2.25% | 47.13% | materially_sensitive |
| `vehicle` | `event_counts.yields` | records (count) | 133.3000 | 131.9000 | 104.3000 | 1.05% | 20.92% | materially_sensitive |

#### mode_pair slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pedestrian_pedestrian` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `vehicle_pedestrian` | `minimum_separation_m` | metres (continuous) | 0.7811 | 0.9912 | 0.6352 | 26.90% | 35.91% | materially_sensitive |
| `vehicle_vehicle` | `minimum_separation_m` | metres (continuous) | -1.7368 | -1.8337 | -1.4860 | 5.58% | 18.96% | materially_sensitive |

#### movement_pair slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `movement:ew_through|movement:ew_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.0500 | 0.1150 | 0.1740 | 130.00% | 51.30% | materially_sensitive |
| `movement:ew_through|movement:ew_through` | `minimum_separation_m` | metres (continuous) | 0.7891 | 0.8951 | 0.9137 | 13.43% | 2.08% | converged |
| `movement:ew_through|movement:ew_through` | `minimum_ttc_s` | seconds (continuous) | 1.2214 | 1.4357 | 1.2337 | 17.54% | 14.07% | materially_sensitive |
| `movement:ew_through|movement:ns_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.5000 | 0.1300 | 0.7260 | 74.00% | 458.46% | materially_sensitive |
| `movement:ew_through|movement:ns_through` | `minimum_separation_m` | metres (continuous) | -1.7368 | -1.8337 | -1.4860 | 5.58% | 18.96% | materially_sensitive |
| `movement:ew_through|movement:ns_through` | `minimum_ttc_s` | seconds (continuous) | 0.0000 | 0.0000 | 0.2471 | - | - | materially_sensitive |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 6.6883 | 6.7070 | 6.6555 | 0.28% | 0.77% | converged |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 6.0763 | 6.2023 | 6.0201 | 2.07% | 2.94% | converged |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | 0.3000 | 0.3200 | 0.4200 | 6.67% | 31.25% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 1.3356 | 1.4693 | 1.5690 | 10.01% | 6.78% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | 1.7493 | 1.7830 | 1.6336 | 1.93% | 8.38% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | 0.1500 | 0.4250 | 0.3480 | 183.33% | 18.12% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 1.0048 | 1.3080 | 0.8494 | 30.17% | 35.06% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 1.4819 | 1.8363 | 1.7122 | 23.91% | 6.76% | materially_sensitive |
| `movement:ns_through|movement:ns_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.0500 | 0.0800 | 0.1280 | 60.00% | 60.00% | materially_sensitive |
| `movement:ns_through|movement:ns_through` | `minimum_separation_m` | metres (continuous) | 0.6571 | 0.6761 | 0.9014 | 2.90% | 33.32% | materially_sensitive |
| `movement:ns_through|movement:ns_through` | `minimum_ttc_s` | seconds (continuous) | 1.1097 | 1.0097 | 1.1818 | 9.01% | 17.04% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | 0.3000 | 0.3600 | 0.5240 | 20.00% | 45.56% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 1.1173 | 1.2997 | 0.7585 | 16.32% | 41.63% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | 1.5845 | 1.4236 | 1.5005 | 10.15% | 5.40% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 0.3200 | 0.3850 | 0.4180 | 20.31% | 8.57% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 1.4538 | 1.3305 | 1.5857 | 8.48% | 19.18% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 1.9331 | 1.9265 | 1.7291 | 0.34% | 10.25% | materially_sensitive |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 6.0147 | 6.2396 | 6.1233 | 3.74% | 1.86% | converged |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 6.6655 | 6.7041 | 6.6967 | 0.58% | 0.11% | converged |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | 2.5000 | 2.1000 | 16.2600 | - | 795.24% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 0.1813 | 1.5930 | 0.9031 | 778.51% | 43.31% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | 0.2744 | 0.2341 | 0.0645 | 23.74% | 82.19% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 5.9333 | 0.9000 | 1.8067 | - | 412.22% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 0.0480 | 0.0491 | 0.0374 | 2.28% | 23.77% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 3.2253 | 2.9905 | 3.2251 | 7.28% | 7.84% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 3.7025 | 4.8409 | 3.8412 | 30.75% | 20.65% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 1.3833 | 0.8100 | 14.2600 | 4.82% | 1618.52% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 0.0995 | 1.1751 | 0.7906 | 1080.70% | 32.73% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 0.1676 | 0.0762 | 0.0734 | 71.19% | 20.20% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 4.4095 | 3.8645 | 3.9281 | 12.36% | 1.65% | converged |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 3.1006 | 3.7971 | 6.1367 | 22.46% | 61.62% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 2.2381 | 1.8994 | 2.4144 | 33.58% | - | inconclusive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | 0.0000 | 1.6900 | 2.1200 | - | - | inconclusive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 0.2882 | 0.2319 | 0.0770 | 19.53% | 66.82% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | 0.3917 | 0.5786 | 0.1236 | 59.34% | 78.64% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | 3.9333 | 0.1500 | 5.8600 | - | - | inconclusive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 0.0529 | 0.0475 | 0.0363 | 10.23% | 23.52% | materially_sensitive |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | 0.5000 | 4.2833 | 2.6867 | - | 43.89% | materially_sensitive |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 0.0777 | 0.4505 | 0.6416 | 479.50% | 42.43% | materially_sensitive |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 0.2536 | 0.5735 | 0.2355 | 126.18% | 44.82% | materially_sensitive |

#### agent_movement slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `movement:ew_through` | `event_counts.collisions` | records (count) | 4.1000 | 4.1000 | 1.2000 | 0.00% | 70.73% | materially_sensitive |
| `movement:ew_through` | `event_counts.control_transitions` | records (count) | 75.0000 | 71.6000 | 65.2000 | 4.53% | 8.94% | materially_sensitive |
| `movement:ew_through` | `event_counts.despawns` | records (count) | 32.8000 | 30.2000 | 24.4000 | 7.93% | 19.21% | materially_sensitive |
| `movement:ew_through` | `event_counts.near_misses` | records (count) | 10.2000 | 9.1000 | 4.3000 | 10.78% | 52.75% | materially_sensitive |
| `movement:ew_through` | `event_counts.queue_events` | records (count) | 122.8000 | 216.2000 | 493.9000 | 76.06% | 128.45% | materially_sensitive |
| `movement:ew_through` | `event_counts.region_entries` | records (count) | 67.8000 | 63.3000 | 49.6000 | 6.64% | 21.64% | materially_sensitive |
| `movement:ew_through` | `event_counts.region_exits` | records (count) | 67.1000 | 62.4000 | 49.5000 | 7.00% | 20.67% | materially_sensitive |
| `movement:ew_through` | `event_counts.spawns` | records (count) | 38.9000 | 36.4000 | 30.5000 | 6.43% | 16.21% | materially_sensitive |
| `movement:ew_through` | `event_counts.violations` | records (count) | 5.2000 | 5.1000 | 2.5000 | 1.92% | 50.98% | materially_sensitive |
| `movement:ew_through` | `event_counts.yields` | records (count) | 72.8000 | 72.1000 | 58.7000 | 0.96% | 18.59% | materially_sensitive |
| `movement:ew_through` | `maximum_queue_duration_s` | seconds (continuous) | 17.8300 | 14.4700 | 27.7660 | 18.84% | 91.89% | materially_sensitive |
| `movement:ew_through` | `maximum_queue_length_agents` | agents (count) | 5.0000 | 4.4000 | 6.1000 | 12.00% | 38.64% | materially_sensitive |
| `movement:ew_through` | `mean_control_delay_s` | seconds (continuous) | 23.2344 | 24.4355 | 29.0850 | 5.17% | 19.03% | materially_sensitive |
| `movement:ew_through` | `mean_queue_duration_s` | seconds (continuous) | 1.0370 | 0.5851 | 0.6570 | 43.58% | 12.28% | materially_sensitive |
| `movement:ew_through` | `mean_stopped_delay_s` | seconds (continuous) | 3.7185 | 4.1439 | 10.9835 | 11.44% | 165.06% | materially_sensitive |
| `movement:ew_through` | `mean_travel_time_s` | seconds (continuous) | 44.8310 | 46.5392 | 54.6696 | 3.81% | 17.47% | materially_sensitive |
| `movement:ew_through` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.1093 | 0.1007 | 0.0813 | 7.93% | 19.21% | materially_sensitive |
| `movement:ew_through` | `total_control_delay_s` | seconds (continuous) | 748.3000 | 723.3500 | 679.4360 | 3.33% | 6.07% | materially_sensitive |
| `movement:ew_through` | `total_stopped_delay_s` | seconds (continuous) | 112.3100 | 113.6100 | 241.8440 | 1.16% | 112.87% | materially_sensitive |
| `movement:ew_through` | `total_travel_time_s` | seconds (continuous) | 1446.3000 | 1377.2300 | 1280.7080 | 4.78% | 7.01% | materially_sensitive |
| `movement:ns_through` | `event_counts.collisions` | records (count) | 2.4000 | 2.0000 | 1.7000 | 16.67% | 15.00% | converged |
| `movement:ns_through` | `event_counts.control_transitions` | records (count) | 61.6000 | 62.7000 | 60.7000 | 1.79% | 3.19% | converged |
| `movement:ns_through` | `event_counts.despawns` | records (count) | 36.4000 | 38.1000 | 21.4000 | 4.67% | 43.83% | materially_sensitive |
| `movement:ns_through` | `event_counts.near_misses` | records (count) | 6.2000 | 5.3000 | 5.3000 | 14.52% | 0.00% | converged |
| `movement:ns_through` | `event_counts.queue_events` | records (count) | 86.9000 | 176.9000 | 348.8000 | 103.57% | 97.17% | materially_sensitive |
| `movement:ns_through` | `event_counts.region_entries` | records (count) | 73.7000 | 77.2000 | 44.2000 | 4.75% | 42.75% | materially_sensitive |
| `movement:ns_through` | `event_counts.region_exits` | records (count) | 73.4000 | 76.8000 | 43.7000 | 4.63% | 43.10% | materially_sensitive |
| `movement:ns_through` | `event_counts.spawns` | records (count) | 42.1000 | 42.9000 | 27.5000 | 1.90% | 35.90% | materially_sensitive |
| `movement:ns_through` | `event_counts.violations` | records (count) | 3.7000 | 3.6000 | 2.1000 | 2.70% | 41.67% | materially_sensitive |
| `movement:ns_through` | `event_counts.yields` | records (count) | 60.5000 | 59.8000 | 45.6000 | 1.16% | 23.75% | materially_sensitive |
| `movement:ns_through` | `maximum_queue_duration_s` | seconds (continuous) | 23.4800 | 21.1100 | 38.8420 | 10.09% | 84.00% | materially_sensitive |
| `movement:ns_through` | `maximum_queue_length_agents` | agents (count) | 5.8000 | 4.8000 | 6.3000 | 17.24% | 31.25% | materially_sensitive |
| `movement:ns_through` | `mean_control_delay_s` | seconds (continuous) | 13.1482 | 13.4949 | 25.7165 | 2.64% | 90.56% | materially_sensitive |
| `movement:ns_through` | `mean_queue_duration_s` | seconds (continuous) | 1.9649 | 0.8088 | 1.7774 | 58.84% | 119.75% | materially_sensitive |
| `movement:ns_through` | `mean_stopped_delay_s` | seconds (continuous) | 3.8171 | 3.6947 | 23.7144 | 3.21% | 541.86% | materially_sensitive |
| `movement:ns_through` | `mean_travel_time_s` | seconds (continuous) | 36.1146 | 36.8274 | 64.3526 | 1.97% | 74.74% | materially_sensitive |
| `movement:ns_through` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.1213 | 0.1270 | 0.0713 | 4.67% | 43.83% | materially_sensitive |
| `movement:ns_through` | `total_control_delay_s` | seconds (continuous) | 469.9600 | 507.8300 | 494.9340 | 8.06% | 2.54% | converged |
| `movement:ns_through` | `total_stopped_delay_s` | seconds (continuous) | 129.2100 | 135.7900 | 406.8820 | 5.09% | 199.64% | materially_sensitive |
| `movement:ns_through` | `total_travel_time_s` | seconds (continuous) | 1294.4000 | 1387.0150 | 1236.5660 | 7.16% | 10.85% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `event_counts.control_transitions` | records (count) | 18.4000 | 15.2000 | 17.4000 | 17.39% | 14.47% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.despawns` | records (count) | 11.5000 | 10.8000 | 11.1000 | 6.09% | 2.78% | converged |
| `pedestrian_route:south_to_east` | `event_counts.near_misses` | records (count) | 18.2000 | 14.9000 | 29.5000 | 18.13% | 97.99% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.queue_events` | records (count) | 70.0000 | 100.2000 | 203.2000 | 43.14% | 102.79% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.region_entries` | records (count) | 13.1000 | 11.6000 | 12.7000 | 11.45% | 9.48% | converged |
| `pedestrian_route:south_to_east` | `event_counts.region_exits` | records (count) | 12.5000 | 11.1000 | 12.5000 | 11.20% | 12.61% | converged |
| `pedestrian_route:south_to_east` | `event_counts.spawns` | records (count) | 13.3000 | 11.7000 | 12.7000 | 12.03% | 8.55% | converged |
| `pedestrian_route:south_to_east` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `maximum_queue_duration_s` | seconds (continuous) | 5.8400 | 8.4950 | 29.3420 | 45.46% | 245.40% | materially_sensitive |
| `pedestrian_route:south_to_east` | `maximum_queue_length_agents` | agents (count) | 2.0000 | 2.0000 | 3.0000 | 0.00% | 50.00% | converged |
| `pedestrian_route:south_to_east` | `mean_control_delay_s` | seconds (continuous) | 3.0678 | 3.6245 | 11.1610 | 18.14% | 207.93% | materially_sensitive |
| `pedestrian_route:south_to_east` | `mean_queue_duration_s` | seconds (continuous) | 0.3470 | 0.2245 | 0.7037 | 35.32% | 213.49% | materially_sensitive |
| `pedestrian_route:south_to_east` | `mean_stopped_delay_s` | seconds (continuous) | 1.4865 | 2.0478 | 8.9357 | 37.76% | 336.35% | materially_sensitive |
| `pedestrian_route:south_to_east` | `mean_travel_time_s` | seconds (continuous) | 29.2581 | 29.3079 | 37.5291 | 0.17% | 28.05% | materially_sensitive |
| `pedestrian_route:south_to_east` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0383 | 0.0360 | 0.0370 | 6.09% | 2.78% | converged |
| `pedestrian_route:south_to_east` | `total_control_delay_s` | seconds (continuous) | 34.1400 | 38.8950 | 129.4800 | 13.93% | 232.90% | materially_sensitive |
| `pedestrian_route:south_to_east` | `total_stopped_delay_s` | seconds (continuous) | 16.4700 | 22.6950 | 104.1220 | 37.80% | 358.79% | materially_sensitive |
| `pedestrian_route:south_to_east` | `total_travel_time_s` | seconds (continuous) | 335.2000 | 317.0500 | 423.0860 | 5.41% | 33.44% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `event_counts.control_transitions` | records (count) | 25.8000 | 21.2000 | 20.8000 | 17.83% | 1.89% | converged |
| `pedestrian_route:south_to_west` | `event_counts.despawns` | records (count) | 12.6000 | 10.2000 | 10.3000 | 19.05% | 0.98% | converged |
| `pedestrian_route:south_to_west` | `event_counts.near_misses` | records (count) | 20.8000 | 14.2000 | 25.3000 | 31.73% | 78.17% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.queue_events` | records (count) | 64.2000 | 88.8000 | 183.4000 | 38.32% | 106.53% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.region_entries` | records (count) | 12.9000 | 10.5000 | 10.4000 | 18.60% | 0.95% | converged |
| `pedestrian_route:south_to_west` | `event_counts.region_exits` | records (count) | 12.8000 | 10.2000 | 10.4000 | 20.31% | 1.96% | converged |
| `pedestrian_route:south_to_west` | `event_counts.spawns` | records (count) | 13.4000 | 11.1000 | 10.9000 | 17.16% | 1.80% | converged |
| `pedestrian_route:south_to_west` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `maximum_queue_duration_s` | seconds (continuous) | 7.8500 | 7.2500 | 26.9620 | 7.64% | 271.89% | materially_sensitive |
| `pedestrian_route:south_to_west` | `maximum_queue_length_agents` | agents (count) | 2.2000 | 1.6000 | 2.7000 | 27.27% | 68.75% | materially_sensitive |
| `pedestrian_route:south_to_west` | `mean_control_delay_s` | seconds (continuous) | 14.5477 | 14.1957 | 25.4171 | 2.42% | 79.05% | materially_sensitive |
| `pedestrian_route:south_to_west` | `mean_queue_duration_s` | seconds (continuous) | 0.3968 | 0.2359 | 0.7831 | 40.54% | 231.91% | materially_sensitive |
| `pedestrian_route:south_to_west` | `mean_stopped_delay_s` | seconds (continuous) | 2.2200 | 2.2988 | 12.1212 | 3.55% | 427.28% | materially_sensitive |
| `pedestrian_route:south_to_west` | `mean_travel_time_s` | seconds (continuous) | 29.0904 | 29.5298 | 40.4664 | 1.51% | 37.04% | materially_sensitive |
| `pedestrian_route:south_to_west` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0420 | 0.0340 | 0.0343 | 19.05% | 0.98% | converged |
| `pedestrian_route:south_to_west` | `total_control_delay_s` | seconds (continuous) | 178.3100 | 144.6350 | 258.0400 | 18.89% | 78.41% | materially_sensitive |
| `pedestrian_route:south_to_west` | `total_stopped_delay_s` | seconds (continuous) | 25.4800 | 21.6050 | 124.6280 | 15.21% | 476.85% | materially_sensitive |
| `pedestrian_route:south_to_west` | `total_travel_time_s` | seconds (continuous) | 363.5100 | 298.3750 | 418.0740 | 17.92% | 40.12% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `event_counts.control_transitions` | records (count) | 14.4000 | 10.9000 | 12.8000 | 24.31% | 17.43% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.despawns` | records (count) | 10.7000 | 12.0000 | 12.3000 | 12.15% | 2.50% | converged |
| `pedestrian_route:west_to_north` | `event_counts.near_misses` | records (count) | 13.5000 | 16.2000 | 21.8000 | 20.00% | 34.57% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.queue_events` | records (count) | 48.4000 | 92.5000 | 226.8000 | 91.12% | 145.19% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.region_entries` | records (count) | 11.6000 | 13.1000 | 14.6000 | 12.93% | 11.45% | converged |
| `pedestrian_route:west_to_north` | `event_counts.region_exits` | records (count) | 11.3000 | 12.9000 | 13.5000 | 14.16% | 4.65% | converged |
| `pedestrian_route:west_to_north` | `event_counts.spawns` | records (count) | 11.6000 | 13.2000 | 14.7000 | 13.79% | 11.36% | converged |
| `pedestrian_route:west_to_north` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `maximum_queue_duration_s` | seconds (continuous) | 3.9400 | 5.5100 | 18.0340 | 39.85% | 227.30% | materially_sensitive |
| `pedestrian_route:west_to_north` | `maximum_queue_length_agents` | agents (count) | 1.5000 | 1.9000 | 2.4000 | 26.67% | 26.32% | converged |
| `pedestrian_route:west_to_north` | `mean_control_delay_s` | seconds (continuous) | 2.8297 | 2.3098 | 6.2785 | 18.37% | 171.82% | materially_sensitive |
| `pedestrian_route:west_to_north` | `mean_queue_duration_s` | seconds (continuous) | 0.3004 | 0.2335 | 0.3139 | 22.26% | 34.42% | materially_sensitive |
| `pedestrian_route:west_to_north` | `mean_stopped_delay_s` | seconds (continuous) | 1.3522 | 1.6440 | 4.9982 | 21.58% | 204.03% | materially_sensitive |
| `pedestrian_route:west_to_north` | `mean_travel_time_s` | seconds (continuous) | 28.2556 | 28.5436 | 32.5049 | 1.02% | 13.88% | materially_sensitive |
| `pedestrian_route:west_to_north` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0357 | 0.0400 | 0.0410 | 12.15% | 2.50% | converged |
| `pedestrian_route:west_to_north` | `total_control_delay_s` | seconds (continuous) | 29.2800 | 27.8000 | 75.8180 | 5.05% | 172.73% | materially_sensitive |
| `pedestrian_route:west_to_north` | `total_stopped_delay_s` | seconds (continuous) | 14.1300 | 19.9850 | 60.2640 | 41.44% | 201.55% | materially_sensitive |
| `pedestrian_route:west_to_north` | `total_travel_time_s` | seconds (continuous) | 302.7600 | 343.9350 | 396.9080 | 13.60% | 15.40% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `event_counts.control_transitions` | records (count) | 19.8000 | 21.7000 | 19.5000 | 9.60% | 10.14% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.despawns` | records (count) | 12.2000 | 11.3000 | 10.8000 | 7.38% | 4.42% | converged |
| `pedestrian_route:west_to_south` | `event_counts.near_misses` | records (count) | 14.8000 | 17.9000 | 22.2000 | 20.95% | 24.02% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.queue_events` | records (count) | 52.8000 | 103.2000 | 239.1000 | 95.45% | 131.69% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.region_entries` | records (count) | 12.5000 | 11.7000 | 11.2000 | 6.40% | 4.27% | converged |
| `pedestrian_route:west_to_south` | `event_counts.region_exits` | records (count) | 12.4000 | 11.5000 | 10.8000 | 7.26% | 6.09% | converged |
| `pedestrian_route:west_to_south` | `event_counts.spawns` | records (count) | 13.3000 | 12.2000 | 11.9000 | 8.27% | 2.46% | converged |
| `pedestrian_route:west_to_south` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `maximum_queue_duration_s` | seconds (continuous) | 2.7000 | 4.6550 | 17.7300 | 72.41% | 280.88% | materially_sensitive |
| `pedestrian_route:west_to_south` | `maximum_queue_length_agents` | agents (count) | 1.6000 | 1.9000 | 3.1000 | 18.75% | 63.16% | materially_sensitive |
| `pedestrian_route:west_to_south` | `mean_control_delay_s` | seconds (continuous) | 8.3612 | 10.5499 | 14.9406 | 26.18% | 41.62% | materially_sensitive |
| `pedestrian_route:west_to_south` | `mean_queue_duration_s` | seconds (continuous) | 0.3046 | 0.2116 | 0.2779 | 30.53% | 31.32% | materially_sensitive |
| `pedestrian_route:west_to_south` | `mean_stopped_delay_s` | seconds (continuous) | 1.2256 | 1.8110 | 5.3616 | 47.76% | 196.05% | materially_sensitive |
| `pedestrian_route:west_to_south` | `mean_travel_time_s` | seconds (continuous) | 27.3121 | 29.6119 | 33.2759 | 8.42% | 12.37% | materially_sensitive |
| `pedestrian_route:west_to_south` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0407 | 0.0377 | 0.0360 | 7.38% | 4.42% | converged |
| `pedestrian_route:west_to_south` | `total_control_delay_s` | seconds (continuous) | 102.6600 | 113.3750 | 159.6220 | 10.44% | 40.79% | materially_sensitive |
| `pedestrian_route:west_to_south` | `total_stopped_delay_s` | seconds (continuous) | 14.7100 | 20.7550 | 57.5000 | 41.09% | 177.04% | materially_sensitive |
| `pedestrian_route:west_to_south` | `total_travel_time_s` | seconds (continuous) | 333.8900 | 332.8750 | 359.0480 | 0.30% | 7.86% | materially_sensitive |

## Counts

| Variant | Metrics | Converged | Materially sensitive | Inconclusive |
| --- | --- | --- | --- | --- |
| `ew_priority` | 254 | 68 | 168 | 18 |
| `ns_priority` | 254 | 62 | 174 | 18 |
