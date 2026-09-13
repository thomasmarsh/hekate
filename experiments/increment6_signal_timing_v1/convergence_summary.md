# increment6_signal_timing_v1 convergence summary

Generated from `convergence_evidence.json` (evidence_version 1, metric_definition_version 2) by `tangle-cli experiment --convergence --summary`. Every number below is that artifact's own.

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
| Control delay of the east-west through movement | agent_movement | `movement:ew_through` | `mean_control_delay_s` | 11.2525 | 14.1770 | 17.5387 | 17.3113 | 24.4355 | 28.0478 | -6.0588 | -10.2585 | -10.5091 | stable |
| Control delay of the north-south through movement | agent_movement | `movement:ns_through` | `mean_control_delay_s` | 20.1826 | 26.5346 | 29.0816 | 12.7018 | 13.4949 | 16.2309 | 7.4808 | 13.0397 | 12.8507 | stable |
| Control delay over the whole run | run | `*` | `operational.run.mean_control_delay_s` | 11.7324 | 14.7338 | 16.5369 | 12.0661 | 13.8245 | 15.7348 | -0.3336 | 0.9093 | 0.8021 | flipped |

## Material sensitivity per metric

Every metric of every slice family the evidence carries, with its across-seed mean at each fidelity, the paired refinement change as a relative change of the coarser fidelity's mean, and the verdict the report reached. `-` is a value the evidence does not carry — not applicable, not observed, or a slice that fidelity does not reach — never a zero.

### ew_priority

#### run slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `*` | `event_counts.by_family.collisions` | records (count) | 2.4000 | 5.9000 | 15.1000 | 145.83% | 155.93% | materially_sensitive |
| `*` | `event_counts.by_family.control_transitions` | records (count) | 96.1000 | 209.0000 | 537.2000 | 117.48% | 157.03% | materially_sensitive |
| `*` | `event_counts.by_family.despawns` | records (count) | 51.0000 | 110.6000 | 290.5000 | 116.86% | 162.66% | materially_sensitive |
| `*` | `event_counts.by_family.near_misses` | records (count) | 39.5000 | 74.9000 | 213.6000 | 89.62% | 185.18% | materially_sensitive |
| `*` | `event_counts.by_family.queue_events` | records (count) | 381.7000 | 770.6000 | 2101.0000 | 101.89% | 172.64% | materially_sensitive |
| `*` | `event_counts.by_family.region_entries` | records (count) | 87.0000 | 183.9000 | 468.2000 | 111.38% | 154.59% | materially_sensitive |
| `*` | `event_counts.by_family.region_exits` | records (count) | 85.1000 | 181.4000 | 465.9000 | 113.16% | 156.84% | materially_sensitive |
| `*` | `event_counts.by_family.spawns` | records (count) | 67.6000 | 126.8000 | 308.0000 | 87.57% | 142.90% | materially_sensitive |
| `*` | `event_counts.by_family.violations` | records (count) | 4.6000 | 7.7000 | 19.7000 | 67.39% | 155.84% | materially_sensitive |
| `*` | `event_counts.by_family.yields` | records (count) | 60.5000 | 133.4000 | 356.6000 | 120.50% | 167.32% | materially_sensitive |
| `*` | `event_counts.by_family_kind.control_transitions.crossing_wait` | records (count) | 34.5000 | 69.6000 | 185.2000 | 101.74% | 166.09% | materially_sensitive |
| `*` | `event_counts.by_family_kind.control_transitions.signal_stop` | records (count) | 61.6000 | 139.4000 | 352.0000 | 126.30% | 152.51% | materially_sensitive |
| `*` | `event_counts.by_family_kind.violations.crossed_against_signal` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `*` | `event_counts.by_family_kind.violations.ran_red_light` | records (count) | 4.6000 | 7.7000 | 19.7000 | 67.39% | 155.84% | materially_sensitive |
| `*` | `event_counts.total` | records (count) | 875.5000 | 1804.2000 | 4775.8000 | 106.08% | 164.70% | materially_sensitive |
| `*` | `minimum_post_encroachment_s` | seconds (continuous) | 0.0550 | 0.0100 | 0.0000 | 81.82% | 100.00% | materially_sensitive |
| `*` | `minimum_separation_m` | metres (continuous) | -1.4250 | -1.8558 | -1.9165 | 30.23% | 3.27% | converged |
| `*` | `minimum_ttc_s` | seconds (continuous) | 0.0101 | 0.0000 | 0.0000 | 100.00% | - | converged |
| `*` | `operational.by_mode.pedestrian.maximum_queue_duration_s` | seconds (continuous) | 8.2200 | 16.2700 | 25.2850 | 97.93% | 55.41% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.maximum_queue_length_agents` | agents (count) | 3.9000 | 4.0000 | 5.7000 | 2.56% | 42.50% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_control_delay_s` | seconds (continuous) | 7.0455 | 7.4961 | 7.7254 | 6.39% | 3.06% | converged |
| `*` | `operational.by_mode.pedestrian.mean_queue_duration_s` | seconds (continuous) | 0.2373 | 0.2619 | 0.2872 | 10.35% | 9.66% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_stopped_delay_s` | seconds (continuous) | 1.7925 | 2.0285 | 2.4085 | 13.16% | 18.73% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_travel_time_s` | seconds (continuous) | 28.8399 | 29.0401 | 29.5759 | 0.69% | 1.85% | converged |
| `*` | `operational.by_mode.pedestrian.throughput_agents_per_s` | agents_per_second (continuous) | 0.1387 | 0.1467 | 0.1599 | 5.77% | 9.00% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.total_control_delay_s` | seconds (continuous) | 146.8750 | 328.1650 | 927.6900 | 123.43% | 182.69% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.total_stopped_delay_s` | seconds (continuous) | 36.1850 | 87.3500 | 290.2000 | 141.40% | 232.23% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.total_travel_time_s` | seconds (continuous) | 601.0750 | 1276.3900 | 3551.0150 | 112.35% | 178.21% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.maximum_queue_duration_s` | seconds (continuous) | 16.4550 | 26.3000 | 37.2800 | 59.83% | 41.75% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.maximum_queue_length_agents` | agents (count) | 6.0000 | 7.1000 | 9.0000 | 18.33% | 26.76% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_control_delay_s` | seconds (continuous) | 15.0741 | 19.6536 | 22.8551 | 30.38% | 16.29% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_queue_duration_s` | seconds (continuous) | 0.6446 | 0.7920 | 0.9261 | 22.87% | 16.94% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_stopped_delay_s` | seconds (continuous) | 3.4575 | 4.1919 | 5.3623 | 21.24% | 27.92% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_travel_time_s` | seconds (continuous) | 35.1678 | 43.1812 | 48.6803 | 22.79% | 12.74% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.throughput_agents_per_s` | agents_per_second (continuous) | 0.2013 | 0.2220 | 0.2275 | 10.26% | 2.46% | converged |
| `*` | `operational.by_mode.vehicle.total_control_delay_s` | seconds (continuous) | 449.0650 | 1296.6550 | 3868.6350 | 188.75% | 198.35% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.total_stopped_delay_s` | seconds (continuous) | 100.9800 | 278.1800 | 903.8100 | 175.48% | 224.90% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.total_travel_time_s` | seconds (continuous) | 1051.4800 | 2851.3450 | 8243.5450 | 171.17% | 189.11% | materially_sensitive |
| `*` | `operational.run.maximum_queue_duration_s` | seconds (continuous) | 16.5250 | 26.3000 | 37.2800 | 59.15% | 41.75% | materially_sensitive |
| `*` | `operational.run.maximum_queue_length_agents` | agents (count) | 7.0000 | 8.6000 | 11.8000 | 22.86% | 37.21% | materially_sensitive |
| `*` | `operational.run.mean_control_delay_s` | seconds (continuous) | 11.7324 | 14.7338 | 16.5369 | 25.58% | 12.24% | materially_sensitive |
| `*` | `operational.run.mean_queue_duration_s` | seconds (continuous) | 0.4180 | 0.5287 | 0.6032 | 26.47% | 14.11% | materially_sensitive |
| `*` | `operational.run.mean_stopped_delay_s` | seconds (continuous) | 2.7007 | 3.3133 | 4.1194 | 22.68% | 24.33% | materially_sensitive |
| `*` | `operational.run.mean_travel_time_s` | seconds (continuous) | 32.4329 | 37.3997 | 40.6525 | 15.31% | 8.70% | materially_sensitive |
| `*` | `operational.run.throughput_agents_per_s` | agents_per_second (continuous) | 0.3400 | 0.3687 | 0.3873 | 8.43% | 5.06% | materially_sensitive |
| `*` | `operational.run.total_control_delay_s` | seconds (continuous) | 595.9400 | 1624.8200 | 4796.3250 | 172.65% | 195.19% | materially_sensitive |
| `*` | `operational.run.total_stopped_delay_s` | seconds (continuous) | 137.1650 | 365.5300 | 1194.0100 | 166.49% | 226.65% | materially_sensitive |
| `*` | `operational.run.total_travel_time_s` | seconds (continuous) | 1652.5550 | 4127.7350 | 11794.5600 | 149.78% | 185.74% | materially_sensitive |

#### mode slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pedestrian` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian` | `event_counts.control_transitions` | records (count) | 34.5000 | 69.6000 | 185.2000 | 101.74% | 166.09% | materially_sensitive |
| `pedestrian` | `event_counts.despawns` | records (count) | 20.8000 | 44.0000 | 119.9000 | 111.54% | 172.50% | materially_sensitive |
| `pedestrian` | `event_counts.near_misses` | records (count) | 33.1000 | 59.7000 | 176.5000 | 80.36% | 195.64% | materially_sensitive |
| `pedestrian` | `event_counts.queue_events` | records (count) | 197.8000 | 370.3000 | 1051.6000 | 87.21% | 183.99% | materially_sensitive |
| `pedestrian` | `event_counts.region_entries` | records (count) | 24.0000 | 46.8000 | 123.3000 | 95.00% | 163.46% | materially_sensitive |
| `pedestrian` | `event_counts.region_exits` | records (count) | 22.1000 | 45.4000 | 122.4000 | 105.43% | 169.60% | materially_sensitive |
| `pedestrian` | `event_counts.spawns` | records (count) | 25.7000 | 48.2000 | 125.3000 | 87.55% | 159.96% | materially_sensitive |
| `pedestrian` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `vehicle` | `event_counts.collisions` | records (count) | 2.4000 | 5.9000 | 15.1000 | 145.83% | 155.93% | materially_sensitive |
| `vehicle` | `event_counts.control_transitions` | records (count) | 61.6000 | 139.4000 | 352.0000 | 126.30% | 152.51% | materially_sensitive |
| `vehicle` | `event_counts.despawns` | records (count) | 30.2000 | 66.6000 | 170.6000 | 120.53% | 156.16% | materially_sensitive |
| `vehicle` | `event_counts.near_misses` | records (count) | 6.4000 | 15.2000 | 37.1000 | 137.50% | 144.08% | materially_sensitive |
| `vehicle` | `event_counts.queue_events` | records (count) | 183.9000 | 400.3000 | 1049.4000 | 117.67% | 162.15% | materially_sensitive |
| `vehicle` | `event_counts.region_entries` | records (count) | 63.0000 | 137.1000 | 344.9000 | 117.62% | 151.57% | materially_sensitive |
| `vehicle` | `event_counts.region_exits` | records (count) | 63.0000 | 136.0000 | 343.5000 | 115.87% | 152.57% | materially_sensitive |
| `vehicle` | `event_counts.spawns` | records (count) | 41.9000 | 78.6000 | 182.7000 | 87.59% | 132.44% | materially_sensitive |
| `vehicle` | `event_counts.violations` | records (count) | 4.6000 | 7.7000 | 19.7000 | 67.39% | 155.84% | materially_sensitive |
| `vehicle` | `event_counts.yields` | records (count) | 60.5000 | 133.4000 | 356.6000 | 120.50% | 167.32% | materially_sensitive |

#### mode_pair slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pedestrian_pedestrian` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `vehicle_pedestrian` | `minimum_separation_m` | metres (continuous) | 1.1782 | 0.8685 | 0.6363 | 26.29% | 26.74% | materially_sensitive |
| `vehicle_vehicle` | `minimum_separation_m` | metres (continuous) | -1.3229 | -1.8558 | -1.9165 | 40.28% | 3.27% | converged |

#### movement_pair slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `movement:ew_through|movement:ew_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.1550 | 0.0400 | 0.0250 | 74.19% | 37.50% | materially_sensitive |
| `movement:ew_through|movement:ew_through` | `minimum_separation_m` | metres (continuous) | 0.9847 | 0.8801 | 0.8791 | 10.62% | 0.12% | converged |
| `movement:ew_through|movement:ew_through` | `minimum_ttc_s` | seconds (continuous) | 1.2505 | 1.2265 | 1.1640 | 1.93% | 5.09% | materially_sensitive |
| `movement:ew_through|movement:ns_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.4250 | 0.1700 | 0.0350 | 60.00% | 79.41% | materially_sensitive |
| `movement:ew_through|movement:ns_through` | `minimum_separation_m` | metres (continuous) | -1.3229 | -1.8558 | -1.9165 | 40.28% | 3.27% | converged |
| `movement:ew_through|movement:ns_through` | `minimum_ttc_s` | seconds (continuous) | 0.4932 | 0.0000 | 0.0000 | 100.00% | - | converged |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 6.7482 | 6.7287 | 6.7010 | 0.29% | 0.41% | converged |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 6.4206 | 6.3168 | 5.9193 | 1.62% | 6.29% | materially_sensitive |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | 0.7750 | 0.3150 | 0.2050 | 59.35% | 34.92% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 1.7389 | 1.5079 | 1.2547 | 13.29% | 16.79% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | 1.9421 | 1.5155 | 1.0722 | 21.97% | 29.25% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | 0.5250 | 0.3200 | 0.2450 | 39.05% | 23.44% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 1.3944 | 1.0765 | 0.8839 | 22.80% | 17.89% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 1.8503 | 1.6628 | 1.3441 | 10.14% | 19.16% | materially_sensitive |
| `movement:ns_through|movement:ns_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.2800 | 0.1550 | 0.0450 | 44.64% | 70.97% | materially_sensitive |
| `movement:ns_through|movement:ns_through` | `minimum_separation_m` | metres (continuous) | 0.7458 | 0.7296 | 0.7284 | 2.17% | 0.17% | converged |
| `movement:ns_through|movement:ns_through` | `minimum_ttc_s` | seconds (continuous) | 1.0937 | 1.0800 | 1.0800 | 1.25% | 0.00% | converged |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | 0.4450 | 0.3700 | 0.2200 | 16.85% | 40.54% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 1.5011 | 1.2947 | 1.0480 | 13.75% | 19.05% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | 2.1852 | 2.0119 | 1.4712 | 7.93% | 26.88% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 0.4850 | 0.3350 | 0.2500 | 30.93% | 25.37% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 1.6075 | 1.4151 | 1.0588 | 11.97% | 25.18% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 2.3818 | 2.0936 | 1.5637 | 12.10% | 25.31% | materially_sensitive |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 6.2958 | 6.1867 | 5.9557 | 1.73% | 3.73% | converged |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 6.7445 | 6.6872 | 6.6496 | 0.85% | 0.56% | converged |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | 4.9000 | 1.5375 | 1.3000 | 0.00% | 0.00% | converged |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 2.1637 | 1.1643 | 0.1961 | 97.39% | 83.16% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | 0.0813 | 0.1096 | 0.2168 | 0.00% | 48.41% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 4.8500 | 7.8000 | 3.6167 | 42.27% | 78.21% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 0.0546 | 0.0502 | 0.0451 | 8.11% | 10.09% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 4.8044 | 3.1134 | 2.7966 | 35.20% | 10.17% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 7.6767 | 4.4004 | 3.1685 | 42.68% | 27.99% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 2.7750 | 2.7000 | 1.8786 | 0.00% | 30.86% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 2.3723 | 1.6926 | 0.1728 | 40.86% | 89.79% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 1.7742 | 1.3209 | 0.3483 | 35.61% | 68.70% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 7.1412 | 4.3838 | 3.8240 | 38.61% | 12.77% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 5.0311 | 3.4603 | 2.5224 | 31.22% | 27.11% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 2.0802 | 1.9751 | 1.5721 | 0.00% | 17.04% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | 1.8625 | 1.9000 | 2.2444 | 0.00% | 0.00% | converged |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 2.0326 | 0.1510 | 0.0515 | 92.57% | 65.88% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | 0.5672 | 0.3897 | 0.0605 | 25.10% | 84.47% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | 5.2900 | 1.1333 | - | 74.29% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 0.0519 | 0.0470 | 0.0450 | 9.54% | 4.31% | converged |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | 1.8500 | 1.8500 | 6.8500 | 0.00% | 0.00% | converged |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 0.5907 | 0.0581 | 0.0500 | 90.16% | 14.01% | materially_sensitive |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 0.6346 | 0.0932 | 0.0620 | 85.31% | 33.52% | materially_sensitive |

#### agent_movement slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `movement:ew_through` | `event_counts.collisions` | records (count) | 0.7000 | 2.0000 | 6.3000 | 185.71% | 215.00% | materially_sensitive |
| `movement:ew_through` | `event_counts.control_transitions` | records (count) | 27.4000 | 62.6000 | 163.2000 | 128.47% | 160.70% | materially_sensitive |
| `movement:ew_through` | `event_counts.despawns` | records (count) | 17.0000 | 35.7000 | 91.4000 | 110.00% | 156.02% | materially_sensitive |
| `movement:ew_through` | `event_counts.near_misses` | records (count) | 2.3000 | 5.7000 | 16.0000 | 147.83% | 180.70% | materially_sensitive |
| `movement:ew_through` | `event_counts.queue_events` | records (count) | 67.8000 | 165.2000 | 457.4000 | 143.66% | 176.88% | materially_sensitive |
| `movement:ew_through` | `event_counts.region_entries` | records (count) | 35.8000 | 74.2000 | 184.0000 | 107.26% | 147.98% | materially_sensitive |
| `movement:ew_through` | `event_counts.region_exits` | records (count) | 35.8000 | 73.4000 | 183.7000 | 105.03% | 150.27% | materially_sensitive |
| `movement:ew_through` | `event_counts.spawns` | records (count) | 22.6000 | 41.7000 | 97.6000 | 84.51% | 134.05% | materially_sensitive |
| `movement:ew_through` | `event_counts.violations` | records (count) | 1.6000 | 3.1000 | 7.6000 | 93.75% | 145.16% | materially_sensitive |
| `movement:ew_through` | `event_counts.yields` | records (count) | 27.8000 | 66.5000 | 175.8000 | 139.21% | 164.36% | materially_sensitive |
| `movement:ew_through` | `maximum_queue_duration_s` | seconds (continuous) | 14.5300 | 22.5950 | 37.0150 | 55.51% | 63.82% | materially_sensitive |
| `movement:ew_through` | `maximum_queue_length_agents` | agents (count) | 4.3000 | 4.8000 | 6.7000 | 11.63% | 39.58% | materially_sensitive |
| `movement:ew_through` | `mean_control_delay_s` | seconds (continuous) | 11.2525 | 14.1770 | 17.5387 | 25.99% | 23.71% | materially_sensitive |
| `movement:ew_through` | `mean_queue_duration_s` | seconds (continuous) | 1.0773 | 1.1732 | 1.2722 | 8.91% | 8.44% | materially_sensitive |
| `movement:ew_through` | `mean_stopped_delay_s` | seconds (continuous) | 3.7182 | 4.4192 | 5.8116 | 18.85% | 31.51% | materially_sensitive |
| `movement:ew_through` | `mean_travel_time_s` | seconds (continuous) | 30.9036 | 37.8568 | 44.6070 | 22.50% | 17.83% | materially_sensitive |
| `movement:ew_through` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.1133 | 0.1190 | 0.1219 | 5.00% | 2.41% | converged |
| `movement:ew_through` | `total_control_delay_s` | seconds (continuous) | 189.6400 | 503.4850 | 1592.2500 | 165.50% | 216.25% | materially_sensitive |
| `movement:ew_through` | `total_stopped_delay_s` | seconds (continuous) | 62.1850 | 155.6250 | 525.7500 | 150.26% | 237.83% | materially_sensitive |
| `movement:ew_through` | `total_travel_time_s` | seconds (continuous) | 523.5850 | 1342.7250 | 4049.8450 | 156.45% | 201.61% | materially_sensitive |
| `movement:ns_through` | `event_counts.collisions` | records (count) | 1.7000 | 3.9000 | 8.8000 | 129.41% | 125.64% | materially_sensitive |
| `movement:ns_through` | `event_counts.control_transitions` | records (count) | 34.2000 | 76.8000 | 188.8000 | 124.56% | 145.83% | materially_sensitive |
| `movement:ns_through` | `event_counts.despawns` | records (count) | 13.2000 | 30.9000 | 79.2000 | 134.09% | 156.31% | materially_sensitive |
| `movement:ns_through` | `event_counts.near_misses` | records (count) | 4.1000 | 9.5000 | 21.1000 | 131.71% | 122.11% | materially_sensitive |
| `movement:ns_through` | `event_counts.queue_events` | records (count) | 116.1000 | 235.1000 | 592.0000 | 102.50% | 151.81% | materially_sensitive |
| `movement:ns_through` | `event_counts.region_entries` | records (count) | 27.2000 | 62.9000 | 160.9000 | 131.25% | 155.80% | materially_sensitive |
| `movement:ns_through` | `event_counts.region_exits` | records (count) | 27.2000 | 62.6000 | 159.8000 | 130.15% | 155.27% | materially_sensitive |
| `movement:ns_through` | `event_counts.spawns` | records (count) | 19.3000 | 36.9000 | 85.1000 | 91.19% | 130.62% | materially_sensitive |
| `movement:ns_through` | `event_counts.violations` | records (count) | 3.0000 | 4.6000 | 12.1000 | 53.33% | 163.04% | materially_sensitive |
| `movement:ns_through` | `event_counts.yields` | records (count) | 32.7000 | 66.9000 | 180.8000 | 104.59% | 170.25% | materially_sensitive |
| `movement:ns_through` | `maximum_queue_duration_s` | seconds (continuous) | 11.8850 | 17.8750 | 31.2200 | 50.40% | 74.66% | materially_sensitive |
| `movement:ns_through` | `maximum_queue_length_agents` | agents (count) | 3.8000 | 5.2000 | 6.4000 | 36.84% | 23.08% | converged |
| `movement:ns_through` | `mean_control_delay_s` | seconds (continuous) | 20.1826 | 26.5346 | 29.0816 | 31.47% | 9.60% | materially_sensitive |
| `movement:ns_through` | `mean_queue_duration_s` | seconds (continuous) | 0.4237 | 0.5432 | 0.6650 | 28.22% | 22.42% | materially_sensitive |
| `movement:ns_through` | `mean_stopped_delay_s` | seconds (continuous) | 3.2809 | 4.3648 | 4.9188 | 33.04% | 12.69% | materially_sensitive |
| `movement:ns_through` | `mean_travel_time_s` | seconds (continuous) | 40.9506 | 50.3850 | 53.5717 | 23.04% | 6.32% | materially_sensitive |
| `movement:ns_through` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0880 | 0.1030 | 0.1056 | 17.05% | 2.52% | converged |
| `movement:ns_through` | `total_control_delay_s` | seconds (continuous) | 259.4250 | 793.1700 | 2276.3850 | 205.74% | 187.00% | materially_sensitive |
| `movement:ns_through` | `total_stopped_delay_s` | seconds (continuous) | 38.7950 | 122.5550 | 378.0600 | 215.90% | 208.48% | materially_sensitive |
| `movement:ns_through` | `total_travel_time_s` | seconds (continuous) | 527.8950 | 1508.6200 | 4193.7000 | 185.78% | 177.98% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `event_counts.control_transitions` | records (count) | 6.0000 | 12.6000 | 32.8000 | 110.00% | 160.32% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.despawns` | records (count) | 5.1000 | 10.8000 | 30.5000 | 111.76% | 182.41% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.near_misses` | records (count) | 7.8000 | 13.8000 | 42.9000 | 76.92% | 210.87% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.queue_events` | records (count) | 51.9000 | 90.1000 | 281.4000 | 73.60% | 212.32% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.region_entries` | records (count) | 6.1000 | 11.6000 | 31.3000 | 90.16% | 169.83% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.region_exits` | records (count) | 5.8000 | 11.1000 | 31.3000 | 91.38% | 181.98% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.spawns` | records (count) | 6.2000 | 11.7000 | 31.3000 | 88.71% | 167.52% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `maximum_queue_duration_s` | seconds (continuous) | 3.9600 | 4.7500 | 11.7550 | 19.95% | 147.47% | materially_sensitive |
| `pedestrian_route:south_to_east` | `maximum_queue_length_agents` | agents (count) | 1.6000 | 1.8000 | 2.4000 | 12.50% | 33.33% | converged |
| `pedestrian_route:south_to_east` | `mean_control_delay_s` | seconds (continuous) | 2.8097 | 2.8028 | 2.4040 | 0.25% | 14.23% | materially_sensitive |
| `pedestrian_route:south_to_east` | `mean_queue_duration_s` | seconds (continuous) | 0.2054 | 0.2228 | 0.2137 | 8.49% | 4.06% | converged |
| `pedestrian_route:south_to_east` | `mean_stopped_delay_s` | seconds (continuous) | 2.0005 | 1.8552 | 1.9322 | 7.26% | 4.15% | converged |
| `pedestrian_route:south_to_east` | `mean_travel_time_s` | seconds (continuous) | 29.3958 | 29.0398 | 28.9532 | 1.21% | 0.30% | converged |
| `pedestrian_route:south_to_east` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0340 | 0.0360 | 0.0407 | 5.88% | 12.96% | materially_sensitive |
| `pedestrian_route:south_to_east` | `total_control_delay_s` | seconds (continuous) | 14.5000 | 30.0350 | 74.3250 | 107.14% | 147.46% | materially_sensitive |
| `pedestrian_route:south_to_east` | `total_stopped_delay_s` | seconds (continuous) | 10.6800 | 20.4900 | 59.4800 | 91.85% | 190.29% | materially_sensitive |
| `pedestrian_route:south_to_east` | `total_travel_time_s` | seconds (continuous) | 150.9250 | 313.7300 | 884.0900 | 107.87% | 181.80% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `event_counts.control_transitions` | records (count) | 7.8000 | 17.4000 | 48.6000 | 123.08% | 179.31% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.despawns` | records (count) | 4.9000 | 10.1000 | 28.9000 | 106.12% | 186.14% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.near_misses` | records (count) | 7.2000 | 12.8000 | 41.9000 | 77.78% | 227.34% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.queue_events` | records (count) | 51.2000 | 89.5000 | 267.7000 | 74.80% | 199.11% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.region_entries` | records (count) | 5.7000 | 10.5000 | 29.3000 | 84.21% | 179.05% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.region_exits` | records (count) | 5.0000 | 10.1000 | 28.9000 | 102.00% | 186.14% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.spawns` | records (count) | 6.2000 | 11.1000 | 30.3000 | 79.03% | 172.97% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `maximum_queue_duration_s` | seconds (continuous) | 3.2150 | 5.9300 | 14.9050 | 84.45% | 151.35% | materially_sensitive |
| `pedestrian_route:south_to_west` | `maximum_queue_length_agents` | agents (count) | 1.2000 | 1.5000 | 2.3000 | 25.00% | 53.33% | converged |
| `pedestrian_route:south_to_west` | `mean_control_delay_s` | seconds (continuous) | 8.4661 | 9.1041 | 9.5521 | 7.54% | 4.92% | converged |
| `pedestrian_route:south_to_west` | `mean_queue_duration_s` | seconds (continuous) | 0.2203 | 0.2483 | 0.2437 | 12.73% | 1.85% | converged |
| `pedestrian_route:south_to_west` | `mean_stopped_delay_s` | seconds (continuous) | 1.8883 | 2.0206 | 2.1845 | 7.00% | 8.11% | materially_sensitive |
| `pedestrian_route:south_to_west` | `mean_travel_time_s` | seconds (continuous) | 28.6215 | 29.2018 | 29.5148 | 2.03% | 1.07% | converged |
| `pedestrian_route:south_to_west` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0327 | 0.0337 | 0.0385 | 3.06% | 14.46% | materially_sensitive |
| `pedestrian_route:south_to_west` | `total_control_delay_s` | seconds (continuous) | 39.8500 | 92.3350 | 278.0900 | 131.71% | 201.18% | materially_sensitive |
| `pedestrian_route:south_to_west` | `total_stopped_delay_s` | seconds (continuous) | 8.9150 | 20.1500 | 63.2550 | 126.02% | 213.92% | materially_sensitive |
| `pedestrian_route:south_to_west` | `total_travel_time_s` | seconds (continuous) | 139.4550 | 293.3850 | 854.2050 | 110.38% | 191.15% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `event_counts.control_transitions` | records (count) | 8.8000 | 16.0000 | 40.8000 | 81.82% | 155.00% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.despawns` | records (count) | 5.8000 | 11.8000 | 30.0000 | 103.45% | 154.24% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.near_misses` | records (count) | 8.8000 | 16.0000 | 44.6000 | 81.82% | 178.75% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.queue_events` | records (count) | 46.0000 | 93.8000 | 244.1000 | 103.91% | 160.23% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.region_entries` | records (count) | 6.7000 | 13.2000 | 31.8000 | 97.01% | 140.91% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.region_exits` | records (count) | 6.3000 | 12.9000 | 31.6000 | 104.76% | 144.96% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.spawns` | records (count) | 6.7000 | 13.2000 | 32.1000 | 97.01% | 143.18% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `maximum_queue_duration_s` | seconds (continuous) | 4.6550 | 8.4850 | 19.0800 | 82.28% | 124.87% | materially_sensitive |
| `pedestrian_route:west_to_north` | `maximum_queue_length_agents` | agents (count) | 1.4000 | 1.8000 | 2.8000 | 28.57% | 55.56% | converged |
| `pedestrian_route:west_to_north` | `mean_control_delay_s` | seconds (continuous) | 2.8919 | 3.4980 | 4.0168 | 20.96% | 14.83% | materially_sensitive |
| `pedestrian_route:west_to_north` | `mean_queue_duration_s` | seconds (continuous) | 0.2652 | 0.2839 | 0.3555 | 7.06% | 25.20% | materially_sensitive |
| `pedestrian_route:west_to_north` | `mean_stopped_delay_s` | seconds (continuous) | 1.5746 | 2.0196 | 2.5114 | 28.26% | 24.35% | materially_sensitive |
| `pedestrian_route:west_to_north` | `mean_travel_time_s` | seconds (continuous) | 28.6561 | 28.9128 | 29.5500 | 0.90% | 2.20% | converged |
| `pedestrian_route:west_to_north` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0387 | 0.0393 | 0.0400 | 1.72% | 1.69% | converged |
| `pedestrian_route:west_to_north` | `total_control_delay_s` | seconds (continuous) | 18.3100 | 39.5150 | 116.2100 | 115.81% | 194.09% | materially_sensitive |
| `pedestrian_route:west_to_north` | `total_stopped_delay_s` | seconds (continuous) | 9.5750 | 22.5300 | 72.7100 | 135.30% | 222.73% | materially_sensitive |
| `pedestrian_route:west_to_north` | `total_travel_time_s` | seconds (continuous) | 167.0200 | 338.8250 | 884.1800 | 102.86% | 160.95% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `event_counts.control_transitions` | records (count) | 11.9000 | 23.6000 | 63.0000 | 98.32% | 166.95% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.despawns` | records (count) | 5.0000 | 11.3000 | 30.5000 | 126.00% | 169.91% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.near_misses` | records (count) | 9.3000 | 17.1000 | 47.1000 | 83.87% | 175.44% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.queue_events` | records (count) | 48.7000 | 96.9000 | 258.4000 | 98.97% | 166.67% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.region_entries` | records (count) | 5.5000 | 11.5000 | 30.9000 | 109.09% | 168.70% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.region_exits` | records (count) | 5.0000 | 11.3000 | 30.6000 | 126.00% | 170.80% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.spawns` | records (count) | 6.6000 | 12.2000 | 31.6000 | 84.85% | 159.02% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `maximum_queue_duration_s` | seconds (continuous) | 2.5750 | 10.0400 | 24.5550 | 289.90% | 144.57% | materially_sensitive |
| `pedestrian_route:west_to_south` | `maximum_queue_length_agents` | agents (count) | 1.6000 | 2.1000 | 2.8000 | 31.25% | 33.33% | converged |
| `pedestrian_route:west_to_south` | `mean_control_delay_s` | seconds (continuous) | 14.7532 | 15.4771 | 15.0259 | 4.91% | 2.92% | converged |
| `pedestrian_route:west_to_south` | `mean_queue_duration_s` | seconds (continuous) | 0.2588 | 0.2929 | 0.3582 | 13.19% | 22.30% | materially_sensitive |
| `pedestrian_route:west_to_south` | `mean_stopped_delay_s` | seconds (continuous) | 1.6551 | 2.4142 | 3.0198 | 45.86% | 25.09% | materially_sensitive |
| `pedestrian_route:west_to_south` | `mean_travel_time_s` | seconds (continuous) | 28.8350 | 29.5173 | 30.2863 | 2.37% | 2.61% | converged |
| `pedestrian_route:west_to_south` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0333 | 0.0377 | 0.0407 | 13.00% | 7.96% | materially_sensitive |
| `pedestrian_route:west_to_south` | `total_control_delay_s` | seconds (continuous) | 74.2150 | 166.2800 | 459.0650 | 124.05% | 176.08% | materially_sensitive |
| `pedestrian_route:west_to_south` | `total_stopped_delay_s` | seconds (continuous) | 7.0150 | 24.1800 | 94.7550 | 244.69% | 291.87% | materially_sensitive |
| `pedestrian_route:west_to_south` | `total_travel_time_s` | seconds (continuous) | 143.6750 | 330.4500 | 928.5400 | 130.00% | 180.99% | materially_sensitive |

### ns_priority

#### run slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `*` | `event_counts.by_family.collisions` | records (count) | 2.7000 | 6.1000 | 15.5000 | 125.93% | 154.10% | materially_sensitive |
| `*` | `event_counts.by_family.control_transitions` | records (count) | 103.8000 | 203.3000 | 524.8000 | 95.86% | 158.14% | materially_sensitive |
| `*` | `event_counts.by_family.despawns` | records (count) | 50.4000 | 112.6000 | 295.0000 | 123.41% | 161.99% | materially_sensitive |
| `*` | `event_counts.by_family.near_misses` | records (count) | 41.5000 | 77.6000 | 213.4000 | 86.99% | 175.00% | materially_sensitive |
| `*` | `event_counts.by_family.queue_events` | records (count) | 379.0000 | 777.8000 | 2042.5000 | 105.22% | 162.60% | materially_sensitive |
| `*` | `event_counts.by_family.region_entries` | records (count) | 86.6000 | 187.4000 | 475.8000 | 116.40% | 153.90% | materially_sensitive |
| `*` | `event_counts.by_family.region_exits` | records (count) | 84.5000 | 184.9000 | 473.9000 | 118.82% | 156.30% | materially_sensitive |
| `*` | `event_counts.by_family.spawns` | records (count) | 66.6000 | 127.5000 | 311.8000 | 91.44% | 144.55% | materially_sensitive |
| `*` | `event_counts.by_family.violations` | records (count) | 4.3000 | 8.7000 | 22.6000 | 102.33% | 159.77% | materially_sensitive |
| `*` | `event_counts.by_family.yields` | records (count) | 58.9000 | 131.9000 | 361.1000 | 123.94% | 173.77% | materially_sensitive |
| `*` | `event_counts.by_family_kind.control_transitions.crossing_wait` | records (count) | 36.9000 | 69.0000 | 180.4000 | 86.99% | 161.45% | materially_sensitive |
| `*` | `event_counts.by_family_kind.control_transitions.signal_stop` | records (count) | 66.9000 | 134.3000 | 344.4000 | 100.75% | 156.44% | materially_sensitive |
| `*` | `event_counts.by_family_kind.violations.crossed_against_signal` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `*` | `event_counts.by_family_kind.violations.ran_red_light` | records (count) | 4.3000 | 8.7000 | 22.6000 | 102.33% | 159.77% | materially_sensitive |
| `*` | `event_counts.total` | records (count) | 878.3000 | 1817.8000 | 4736.4000 | 106.97% | 160.56% | materially_sensitive |
| `*` | `minimum_post_encroachment_s` | seconds (continuous) | 0.0700 | 0.0200 | 0.0050 | 71.43% | 75.00% | materially_sensitive |
| `*` | `minimum_separation_m` | metres (continuous) | -1.4651 | -1.8337 | -1.9037 | 25.16% | 3.82% | converged |
| `*` | `minimum_ttc_s` | seconds (continuous) | 0.0044 | 0.0000 | 0.0000 | 100.00% | - | converged |
| `*` | `operational.by_mode.pedestrian.maximum_queue_duration_s` | seconds (continuous) | 14.1750 | 14.2100 | 22.6850 | 0.25% | 59.64% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.maximum_queue_length_agents` | agents (count) | 4.2000 | 4.4000 | 4.9000 | 4.76% | 11.36% | converged |
| `*` | `operational.by_mode.pedestrian.mean_control_delay_s` | seconds (continuous) | 8.1348 | 7.2832 | 7.4421 | 10.47% | 2.18% | converged |
| `*` | `operational.by_mode.pedestrian.mean_queue_duration_s` | seconds (continuous) | 0.2945 | 0.2264 | 0.2477 | 23.13% | 9.43% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_stopped_delay_s` | seconds (continuous) | 2.3811 | 1.8894 | 2.0642 | 20.65% | 9.25% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.mean_travel_time_s` | seconds (continuous) | 29.8144 | 29.1247 | 29.2450 | 2.31% | 0.41% | converged |
| `*` | `operational.by_mode.pedestrian.throughput_agents_per_s` | agents_per_second (continuous) | 0.1393 | 0.1477 | 0.1605 | 5.98% | 8.71% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.total_control_delay_s` | seconds (continuous) | 167.8500 | 324.7050 | 898.3350 | 93.45% | 176.66% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.total_stopped_delay_s` | seconds (continuous) | 48.4450 | 85.0400 | 250.0100 | 75.54% | 193.99% | materially_sensitive |
| `*` | `operational.by_mode.pedestrian.total_travel_time_s` | seconds (continuous) | 622.1650 | 1292.2350 | 3525.1700 | 107.70% | 172.80% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.maximum_queue_duration_s` | seconds (continuous) | 22.2250 | 23.9650 | 30.6550 | 7.83% | 27.92% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.maximum_queue_length_agents` | agents (count) | 6.5000 | 7.1000 | 8.3000 | 9.23% | 16.90% | converged |
| `*` | `operational.by_mode.vehicle.mean_control_delay_s` | seconds (continuous) | 14.8334 | 18.0510 | 21.5085 | 21.69% | 19.15% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_queue_duration_s` | seconds (continuous) | 0.8511 | 0.6785 | 0.7937 | 20.28% | 16.99% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_stopped_delay_s` | seconds (continuous) | 3.9757 | 3.6986 | 4.4987 | 6.97% | 21.63% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.mean_travel_time_s` | seconds (continuous) | 35.8027 | 40.5374 | 46.3355 | 13.22% | 14.30% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.throughput_agents_per_s` | agents_per_second (continuous) | 0.1967 | 0.2277 | 0.2328 | 15.76% | 2.25% | converged |
| `*` | `operational.by_mode.vehicle.total_control_delay_s` | seconds (continuous) | 436.6300 | 1231.1800 | 3739.5050 | 181.97% | 203.73% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.total_stopped_delay_s` | seconds (continuous) | 114.9300 | 249.4000 | 776.4800 | 117.00% | 211.34% | materially_sensitive |
| `*` | `operational.by_mode.vehicle.total_travel_time_s` | seconds (continuous) | 1051.3600 | 2764.2450 | 8057.0450 | 162.92% | 191.47% | materially_sensitive |
| `*` | `operational.run.maximum_queue_duration_s` | seconds (continuous) | 22.2750 | 23.9650 | 30.6550 | 7.59% | 27.92% | materially_sensitive |
| `*` | `operational.run.maximum_queue_length_agents` | agents (count) | 9.4000 | 9.8000 | 11.6000 | 4.26% | 18.37% | materially_sensitive |
| `*` | `operational.run.mean_control_delay_s` | seconds (continuous) | 12.0661 | 13.8245 | 15.7348 | 14.57% | 13.82% | materially_sensitive |
| `*` | `operational.run.mean_queue_duration_s` | seconds (continuous) | 0.5568 | 0.4507 | 0.5192 | 19.06% | 15.21% | materially_sensitive |
| `*` | `operational.run.mean_stopped_delay_s` | seconds (continuous) | 3.3227 | 2.9644 | 3.4873 | 10.78% | 17.64% | materially_sensitive |
| `*` | `operational.run.mean_travel_time_s` | seconds (continuous) | 33.3606 | 36.0368 | 39.2855 | 8.02% | 9.01% | materially_sensitive |
| `*` | `operational.run.throughput_agents_per_s` | agents_per_second (continuous) | 0.3360 | 0.3753 | 0.3933 | 11.71% | 4.80% | converged |
| `*` | `operational.run.total_control_delay_s` | seconds (continuous) | 604.4800 | 1555.8850 | 4637.8400 | 157.39% | 198.08% | materially_sensitive |
| `*` | `operational.run.total_stopped_delay_s` | seconds (continuous) | 163.3750 | 334.4400 | 1026.4900 | 104.71% | 206.93% | materially_sensitive |
| `*` | `operational.run.total_travel_time_s` | seconds (continuous) | 1673.5250 | 4056.4800 | 11582.2150 | 142.39% | 185.52% | materially_sensitive |

#### mode slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pedestrian` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian` | `event_counts.control_transitions` | records (count) | 36.9000 | 69.0000 | 180.4000 | 86.99% | 161.45% | materially_sensitive |
| `pedestrian` | `event_counts.despawns` | records (count) | 20.9000 | 44.3000 | 120.4000 | 111.96% | 171.78% | materially_sensitive |
| `pedestrian` | `event_counts.near_misses` | records (count) | 35.0000 | 63.2000 | 176.4000 | 80.57% | 179.11% | materially_sensitive |
| `pedestrian` | `event_counts.queue_events` | records (count) | 195.5000 | 384.7000 | 1023.4000 | 96.78% | 166.03% | materially_sensitive |
| `pedestrian` | `event_counts.region_entries` | records (count) | 24.1000 | 46.9000 | 123.3000 | 94.61% | 162.90% | materially_sensitive |
| `pedestrian` | `event_counts.region_exits` | records (count) | 22.9000 | 45.7000 | 122.4000 | 99.56% | 167.83% | materially_sensitive |
| `pedestrian` | `event_counts.spawns` | records (count) | 25.7000 | 48.2000 | 125.3000 | 87.55% | 159.96% | materially_sensitive |
| `pedestrian` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `vehicle` | `event_counts.collisions` | records (count) | 2.7000 | 6.1000 | 15.5000 | 125.93% | 154.10% | materially_sensitive |
| `vehicle` | `event_counts.control_transitions` | records (count) | 66.9000 | 134.3000 | 344.4000 | 100.75% | 156.44% | materially_sensitive |
| `vehicle` | `event_counts.despawns` | records (count) | 29.5000 | 68.3000 | 174.6000 | 131.53% | 155.64% | materially_sensitive |
| `vehicle` | `event_counts.near_misses` | records (count) | 6.5000 | 14.4000 | 37.0000 | 121.54% | 156.94% | materially_sensitive |
| `vehicle` | `event_counts.queue_events` | records (count) | 183.5000 | 393.1000 | 1019.1000 | 114.22% | 159.25% | materially_sensitive |
| `vehicle` | `event_counts.region_entries` | records (count) | 62.5000 | 140.5000 | 352.5000 | 124.80% | 150.89% | materially_sensitive |
| `vehicle` | `event_counts.region_exits` | records (count) | 61.6000 | 139.2000 | 351.5000 | 125.97% | 152.51% | materially_sensitive |
| `vehicle` | `event_counts.spawns` | records (count) | 40.9000 | 79.3000 | 186.5000 | 93.89% | 135.18% | materially_sensitive |
| `vehicle` | `event_counts.violations` | records (count) | 4.3000 | 8.7000 | 22.6000 | 102.33% | 159.77% | materially_sensitive |
| `vehicle` | `event_counts.yields` | records (count) | 58.9000 | 131.9000 | 361.1000 | 123.94% | 173.77% | materially_sensitive |

#### mode_pair slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pedestrian_pedestrian` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `vehicle_pedestrian` | `minimum_separation_m` | metres (continuous) | 1.1121 | 0.9912 | 0.7493 | 10.87% | 24.41% | materially_sensitive |
| `vehicle_vehicle` | `minimum_separation_m` | metres (continuous) | -1.3700 | -1.8337 | -1.9037 | 33.84% | 3.82% | converged |

#### movement_pair slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `movement:ew_through|movement:ew_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.2250 | 0.1150 | 0.0450 | 48.89% | 60.87% | materially_sensitive |
| `movement:ew_through|movement:ew_through` | `minimum_separation_m` | metres (continuous) | 1.0062 | 0.8951 | 0.8945 | 11.05% | 0.06% | converged |
| `movement:ew_through|movement:ew_through` | `minimum_ttc_s` | seconds (continuous) | 1.4358 | 1.4357 | 1.3202 | 0.01% | 8.04% | materially_sensitive |
| `movement:ew_through|movement:ns_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.7000 | 0.1300 | 0.0850 | 81.43% | 34.62% | materially_sensitive |
| `movement:ew_through|movement:ns_through` | `minimum_separation_m` | metres (continuous) | -1.2569 | -1.8337 | -1.9037 | 45.88% | 3.82% | converged |
| `movement:ew_through|movement:ns_through` | `minimum_ttc_s` | seconds (continuous) | 0.4523 | 0.0000 | 0.0000 | 100.00% | - | converged |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 6.7440 | 6.7070 | 6.6392 | 0.55% | 1.01% | converged |
| `movement:ew_through|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 6.3292 | 6.2023 | 5.8448 | 2.01% | 5.76% | materially_sensitive |
| `movement:ew_through|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | 0.5650 | 0.3200 | 0.1850 | 43.36% | 42.19% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 1.5818 | 1.4693 | 1.3069 | 7.11% | 11.05% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | 2.1273 | 1.7830 | 1.5248 | 16.18% | 14.49% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | 0.5750 | 0.4250 | 0.2850 | 26.09% | 32.94% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 1.4973 | 1.3080 | 0.7734 | 12.64% | 40.87% | materially_sensitive |
| `movement:ew_through|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 2.1490 | 1.8363 | 1.5058 | 14.55% | 18.00% | materially_sensitive |
| `movement:ns_through|movement:ns_through` | `minimum_post_encroachment_s` | seconds (continuous) | 0.1200 | 0.0800 | 0.0200 | 33.33% | 75.00% | materially_sensitive |
| `movement:ns_through|movement:ns_through` | `minimum_separation_m` | metres (continuous) | 0.8660 | 0.6761 | 0.6218 | 21.92% | 8.03% | materially_sensitive |
| `movement:ns_through|movement:ns_through` | `minimum_ttc_s` | seconds (continuous) | 1.1863 | 1.0097 | 0.8789 | 14.88% | 12.96% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | 0.4900 | 0.3600 | 0.1450 | 26.53% | 59.72% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 1.3513 | 1.2997 | 0.9810 | 3.82% | 24.52% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | 1.6553 | 1.4236 | 1.0625 | 14.00% | 25.36% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 0.5600 | 0.3850 | 0.2100 | 31.25% | 45.45% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 1.5325 | 1.3305 | 1.2495 | 13.18% | 6.09% | materially_sensitive |
| `movement:ns_through|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 2.4791 | 1.9265 | 1.2295 | 22.29% | 36.18% | materially_sensitive |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 6.3378 | 6.2396 | 6.0696 | 1.55% | 2.72% | converged |
| `movement:ns_through|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 6.7316 | 6.7041 | 6.6460 | 0.41% | 0.87% | converged |
| `movement:ns_through|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_post_encroachment_s` | seconds (continuous) | 3.2000 | 2.1000 | 0.7800 | 0.00% | 58.33% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_separation_m` | metres (continuous) | 1.0483 | 1.5930 | 0.0500 | 86.05% | 96.86% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_east` | `minimum_ttc_s` | seconds (continuous) | 0.9940 | 0.2341 | 0.0604 | 76.44% | 77.04% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 3.6250 | 0.9000 | 1.1563 | 63.45% | 12.50% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `pedestrian_route:south_to_east|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 0.0524 | 0.0491 | 0.0449 | 6.34% | 8.53% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 4.4078 | 2.9905 | 2.7767 | 32.15% | 7.15% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 6.8710 | 4.8409 | 3.1022 | 29.55% | 35.92% | materially_sensitive |
| `pedestrian_route:south_to_east|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_post_encroachment_s` | seconds (continuous) | 0.8167 | 0.8100 | 0.4125 | 0.00% | 45.68% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_separation_m` | metres (continuous) | 0.9879 | 1.1751 | 0.0505 | 16.07% | 95.71% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:south_to_west` | `minimum_ttc_s` | seconds (continuous) | 0.3358 | 0.0762 | 0.0589 | 77.31% | 23.93% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 6.3456 | 3.8645 | 3.1377 | 39.10% | 18.81% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | - | - | - | - | - | inconclusive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 6.2017 | 3.7971 | 2.6983 | 38.77% | 28.94% | materially_sensitive |
| `pedestrian_route:south_to_west|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 1.2016 | 1.8994 | 1.6187 | 0.00% | 5.19% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_post_encroachment_s` | seconds (continuous) | 2.4667 | 1.6900 | 0.3833 | 0.00% | 73.96% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_separation_m` | metres (continuous) | 2.2141 | 0.2319 | 0.0567 | 89.52% | 75.55% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_north` | `minimum_ttc_s` | seconds (continuous) | 0.7800 | 0.5786 | 0.0976 | 10.83% | 83.14% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | 0.3000 | 0.1500 | 0.1750 | 0.00% | 66.67% | materially_sensitive |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 0.0500 | 0.0500 | 0.0500 | 0.00% | 0.00% | converged |
| `pedestrian_route:west_to_north|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 0.0528 | 0.0475 | 0.0444 | 10.05% | 6.61% | materially_sensitive |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_post_encroachment_s` | seconds (continuous) | 9.3500 | 4.2833 | 3.3300 | 0.00% | 6.23% | materially_sensitive |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_separation_m` | metres (continuous) | 0.9270 | 0.4505 | 0.0500 | 51.40% | 88.90% | materially_sensitive |
| `pedestrian_route:west_to_south|pedestrian_route:west_to_south` | `minimum_ttc_s` | seconds (continuous) | 0.5092 | 0.5735 | 0.0762 | 3.26% | 86.72% | materially_sensitive |

#### agent_movement slices

| Slice | Metric | Unit (class) | Fast | Standard | Fine | Fast -> Standard | Standard -> Fine | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `movement:ew_through` | `event_counts.collisions` | records (count) | 1.5000 | 4.1000 | 10.6000 | 173.33% | 158.54% | materially_sensitive |
| `movement:ew_through` | `event_counts.control_transitions` | records (count) | 33.5000 | 71.6000 | 185.2000 | 113.73% | 158.66% | materially_sensitive |
| `movement:ew_through` | `event_counts.despawns` | records (count) | 14.7000 | 30.2000 | 79.3000 | 105.44% | 162.58% | materially_sensitive |
| `movement:ew_through` | `event_counts.near_misses` | records (count) | 3.3000 | 9.1000 | 24.4000 | 175.76% | 168.13% | materially_sensitive |
| `movement:ew_through` | `event_counts.queue_events` | records (count) | 94.5000 | 216.2000 | 586.7000 | 128.78% | 171.37% | materially_sensitive |
| `movement:ew_through` | `event_counts.region_entries` | records (count) | 30.6000 | 63.3000 | 160.0000 | 106.86% | 152.76% | materially_sensitive |
| `movement:ew_through` | `event_counts.region_exits` | records (count) | 30.1000 | 62.4000 | 159.7000 | 107.31% | 155.93% | materially_sensitive |
| `movement:ew_through` | `event_counts.spawns` | records (count) | 20.6000 | 36.4000 | 85.4000 | 76.70% | 134.62% | materially_sensitive |
| `movement:ew_through` | `event_counts.violations` | records (count) | 2.4000 | 5.1000 | 12.7000 | 112.50% | 149.02% | materially_sensitive |
| `movement:ew_through` | `event_counts.yields` | records (count) | 30.8000 | 72.1000 | 191.8000 | 134.09% | 166.02% | materially_sensitive |
| `movement:ew_through` | `maximum_queue_duration_s` | seconds (continuous) | 12.7150 | 14.4700 | 23.4000 | 13.80% | 61.71% | materially_sensitive |
| `movement:ew_through` | `maximum_queue_length_agents` | agents (count) | 3.6000 | 4.4000 | 6.4000 | 22.22% | 45.45% | materially_sensitive |
| `movement:ew_through` | `mean_control_delay_s` | seconds (continuous) | 17.3113 | 24.4355 | 28.0478 | 41.15% | 14.78% | materially_sensitive |
| `movement:ew_through` | `mean_queue_duration_s` | seconds (continuous) | 0.7620 | 0.5851 | 0.6399 | 23.22% | 9.36% | materially_sensitive |
| `movement:ew_through` | `mean_stopped_delay_s` | seconds (continuous) | 3.3847 | 4.1439 | 4.5105 | 22.43% | 8.85% | materially_sensitive |
| `movement:ew_through` | `mean_travel_time_s` | seconds (continuous) | 35.6226 | 46.5392 | 52.0786 | 30.65% | 11.90% | materially_sensitive |
| `movement:ew_through` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0980 | 0.1007 | 0.1057 | 2.72% | 5.03% | materially_sensitive |
| `movement:ew_through` | `total_control_delay_s` | seconds (continuous) | 254.6750 | 723.3500 | 2201.4100 | 184.03% | 204.34% | materially_sensitive |
| `movement:ew_through` | `total_stopped_delay_s` | seconds (continuous) | 49.2750 | 113.6100 | 348.4100 | 130.56% | 206.67% | materially_sensitive |
| `movement:ew_through` | `total_travel_time_s` | seconds (continuous) | 522.7950 | 1377.2300 | 4089.1300 | 163.44% | 196.91% | materially_sensitive |
| `movement:ns_through` | `event_counts.collisions` | records (count) | 1.2000 | 2.0000 | 4.9000 | 66.67% | 145.00% | materially_sensitive |
| `movement:ns_through` | `event_counts.control_transitions` | records (count) | 33.4000 | 62.7000 | 159.2000 | 87.72% | 153.91% | materially_sensitive |
| `movement:ns_through` | `event_counts.despawns` | records (count) | 14.8000 | 38.1000 | 95.3000 | 157.43% | 150.13% | materially_sensitive |
| `movement:ns_through` | `event_counts.near_misses` | records (count) | 3.2000 | 5.3000 | 12.6000 | 65.62% | 137.74% | materially_sensitive |
| `movement:ns_through` | `event_counts.queue_events` | records (count) | 89.0000 | 176.9000 | 432.4000 | 98.76% | 144.43% | materially_sensitive |
| `movement:ns_through` | `event_counts.region_entries` | records (count) | 31.9000 | 77.2000 | 192.5000 | 142.01% | 149.35% | materially_sensitive |
| `movement:ns_through` | `event_counts.region_exits` | records (count) | 31.5000 | 76.8000 | 191.8000 | 143.81% | 149.74% | materially_sensitive |
| `movement:ns_through` | `event_counts.spawns` | records (count) | 20.3000 | 42.9000 | 101.1000 | 111.33% | 135.66% | materially_sensitive |
| `movement:ns_through` | `event_counts.violations` | records (count) | 1.9000 | 3.6000 | 9.9000 | 89.47% | 175.00% | materially_sensitive |
| `movement:ns_through` | `event_counts.yields` | records (count) | 28.1000 | 59.8000 | 169.3000 | 112.81% | 183.11% | materially_sensitive |
| `movement:ns_through` | `maximum_queue_duration_s` | seconds (continuous) | 17.3600 | 21.1100 | 29.3450 | 21.60% | 39.01% | materially_sensitive |
| `movement:ns_through` | `maximum_queue_length_agents` | agents (count) | 4.3000 | 4.8000 | 6.3000 | 11.63% | 31.25% | materially_sensitive |
| `movement:ns_through` | `mean_control_delay_s` | seconds (continuous) | 12.7018 | 13.4949 | 16.2309 | 6.24% | 20.27% | materially_sensitive |
| `movement:ns_through` | `mean_queue_duration_s` | seconds (continuous) | 1.1569 | 0.8088 | 1.0057 | 30.08% | 24.34% | materially_sensitive |
| `movement:ns_through` | `mean_stopped_delay_s` | seconds (continuous) | 4.8955 | 3.6947 | 4.5743 | 24.53% | 23.81% | materially_sensitive |
| `movement:ns_through` | `mean_travel_time_s` | seconds (continuous) | 36.6126 | 36.8274 | 41.8434 | 0.59% | 13.62% | materially_sensitive |
| `movement:ns_through` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0987 | 0.1270 | 0.1271 | 28.72% | 0.05% | converged |
| `movement:ns_through` | `total_control_delay_s` | seconds (continuous) | 181.9550 | 507.8300 | 1538.0950 | 179.10% | 202.88% | materially_sensitive |
| `movement:ns_through` | `total_stopped_delay_s` | seconds (continuous) | 65.6550 | 135.7900 | 428.0700 | 106.82% | 215.24% | materially_sensitive |
| `movement:ns_through` | `total_travel_time_s` | seconds (continuous) | 528.5650 | 1387.0150 | 3967.9150 | 162.41% | 186.08% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `event_counts.control_transitions` | records (count) | 7.9000 | 15.2000 | 40.4000 | 92.41% | 165.79% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.despawns` | records (count) | 5.0000 | 10.8000 | 30.5000 | 116.00% | 182.41% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.near_misses` | records (count) | 8.2000 | 14.9000 | 45.9000 | 81.71% | 208.05% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.queue_events` | records (count) | 56.1000 | 100.2000 | 271.6000 | 78.61% | 171.06% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.region_entries` | records (count) | 6.1000 | 11.6000 | 31.3000 | 90.16% | 169.83% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.region_exits` | records (count) | 5.9000 | 11.1000 | 31.1000 | 88.14% | 180.18% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.spawns` | records (count) | 6.2000 | 11.7000 | 31.3000 | 88.71% | 167.52% | materially_sensitive |
| `pedestrian_route:south_to_east` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_east` | `maximum_queue_duration_s` | seconds (continuous) | 8.2900 | 8.4950 | 17.4050 | 2.47% | 104.89% | materially_sensitive |
| `pedestrian_route:south_to_east` | `maximum_queue_length_agents` | agents (count) | 1.8000 | 2.0000 | 2.1000 | 11.11% | 5.00% | converged |
| `pedestrian_route:south_to_east` | `mean_control_delay_s` | seconds (continuous) | 5.5898 | 3.6245 | 3.6306 | 35.16% | 0.17% | converged |
| `pedestrian_route:south_to_east` | `mean_queue_duration_s` | seconds (continuous) | 0.2870 | 0.2245 | 0.2607 | 21.77% | 16.14% | materially_sensitive |
| `pedestrian_route:south_to_east` | `mean_stopped_delay_s` | seconds (continuous) | 3.8140 | 2.0478 | 2.3003 | 46.31% | 12.33% | materially_sensitive |
| `pedestrian_route:south_to_east` | `mean_travel_time_s` | seconds (continuous) | 31.9617 | 29.3079 | 29.6044 | 8.30% | 1.01% | converged |
| `pedestrian_route:south_to_east` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0333 | 0.0360 | 0.0407 | 8.00% | 12.96% | materially_sensitive |
| `pedestrian_route:south_to_east` | `total_control_delay_s` | seconds (continuous) | 22.8850 | 38.8950 | 108.9700 | 69.96% | 180.16% | materially_sensitive |
| `pedestrian_route:south_to_east` | `total_stopped_delay_s` | seconds (continuous) | 15.3950 | 22.6950 | 68.6250 | 47.42% | 202.38% | materially_sensitive |
| `pedestrian_route:south_to_east` | `total_travel_time_s` | seconds (continuous) | 155.9150 | 317.0500 | 899.8700 | 103.35% | 183.83% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `event_counts.control_transitions` | records (count) | 11.4000 | 21.2000 | 59.2000 | 85.96% | 179.25% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.despawns` | records (count) | 5.2000 | 10.2000 | 28.8000 | 96.15% | 182.35% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.near_misses` | records (count) | 7.8000 | 14.2000 | 45.1000 | 82.05% | 217.61% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.queue_events` | records (count) | 49.7000 | 88.8000 | 242.6000 | 78.67% | 173.20% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.region_entries` | records (count) | 5.7000 | 10.5000 | 29.3000 | 84.21% | 179.05% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.region_exits` | records (count) | 5.3000 | 10.2000 | 29.0000 | 92.45% | 184.31% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.spawns` | records (count) | 6.2000 | 11.1000 | 30.3000 | 79.03% | 172.97% | materially_sensitive |
| `pedestrian_route:south_to_west` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:south_to_west` | `maximum_queue_duration_s` | seconds (continuous) | 6.4650 | 7.2500 | 17.4800 | 12.14% | 141.10% | materially_sensitive |
| `pedestrian_route:south_to_west` | `maximum_queue_length_agents` | agents (count) | 1.5000 | 1.6000 | 2.6000 | 6.67% | 62.50% | converged |
| `pedestrian_route:south_to_west` | `mean_control_delay_s` | seconds (continuous) | 14.5615 | 14.1957 | 14.5696 | 2.51% | 2.63% | converged |
| `pedestrian_route:south_to_west` | `mean_queue_duration_s` | seconds (continuous) | 0.2928 | 0.2359 | 0.3150 | 19.42% | 33.49% | materially_sensitive |
| `pedestrian_route:south_to_west` | `mean_stopped_delay_s` | seconds (continuous) | 2.9447 | 2.2988 | 2.5820 | 21.93% | 12.32% | materially_sensitive |
| `pedestrian_route:south_to_west` | `mean_travel_time_s` | seconds (continuous) | 30.4458 | 29.5298 | 29.8028 | 3.01% | 0.92% | converged |
| `pedestrian_route:south_to_west` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0347 | 0.0340 | 0.0384 | 1.92% | 12.94% | materially_sensitive |
| `pedestrian_route:south_to_west` | `total_control_delay_s` | seconds (continuous) | 70.5300 | 144.6350 | 422.7100 | 105.07% | 192.26% | materially_sensitive |
| `pedestrian_route:south_to_west` | `total_stopped_delay_s` | seconds (continuous) | 14.1050 | 21.6050 | 73.8200 | 53.17% | 241.68% | materially_sensitive |
| `pedestrian_route:south_to_west` | `total_travel_time_s` | seconds (continuous) | 154.1650 | 298.3750 | 858.8200 | 93.54% | 187.83% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `event_counts.control_transitions` | records (count) | 5.4000 | 10.9000 | 29.2000 | 101.85% | 167.89% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.despawns` | records (count) | 5.5000 | 12.0000 | 30.6000 | 118.18% | 155.00% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.near_misses` | records (count) | 8.6000 | 16.2000 | 41.2000 | 88.37% | 154.32% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.queue_events` | records (count) | 39.4000 | 92.5000 | 239.9000 | 134.77% | 159.35% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.region_entries` | records (count) | 6.7000 | 13.1000 | 31.8000 | 95.52% | 142.75% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.region_exits` | records (count) | 6.4000 | 12.9000 | 31.6000 | 101.56% | 144.96% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.spawns` | records (count) | 6.7000 | 13.2000 | 32.1000 | 97.01% | 143.18% | materially_sensitive |
| `pedestrian_route:west_to_north` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_north` | `maximum_queue_duration_s` | seconds (continuous) | 4.7350 | 5.5100 | 9.8200 | 16.37% | 78.22% | materially_sensitive |
| `pedestrian_route:west_to_north` | `maximum_queue_length_agents` | agents (count) | 1.5000 | 1.9000 | 2.0000 | 26.67% | 5.26% | converged |
| `pedestrian_route:west_to_north` | `mean_control_delay_s` | seconds (continuous) | 1.7297 | 2.3098 | 2.4235 | 33.54% | 4.92% | converged |
| `pedestrian_route:west_to_north` | `mean_queue_duration_s` | seconds (continuous) | 0.4172 | 0.2335 | 0.2147 | 44.02% | 8.08% | materially_sensitive |
| `pedestrian_route:west_to_north` | `mean_stopped_delay_s` | seconds (continuous) | 1.3910 | 1.6440 | 1.6659 | 18.18% | 1.33% | converged |
| `pedestrian_route:west_to_north` | `mean_travel_time_s` | seconds (continuous) | 27.9935 | 28.5436 | 28.4721 | 1.96% | 0.25% | converged |
| `pedestrian_route:west_to_north` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0367 | 0.0400 | 0.0408 | 9.09% | 2.00% | converged |
| `pedestrian_route:west_to_north` | `total_control_delay_s` | seconds (continuous) | 10.0000 | 27.8000 | 72.9200 | 178.00% | 162.30% | materially_sensitive |
| `pedestrian_route:west_to_north` | `total_stopped_delay_s` | seconds (continuous) | 7.8950 | 19.9850 | 50.6450 | 153.13% | 153.42% | materially_sensitive |
| `pedestrian_route:west_to_north` | `total_travel_time_s` | seconds (continuous) | 155.5600 | 343.9350 | 871.5650 | 121.09% | 153.41% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.collisions` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `event_counts.control_transitions` | records (count) | 12.2000 | 21.7000 | 51.6000 | 77.87% | 137.79% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.despawns` | records (count) | 5.2000 | 11.3000 | 30.5000 | 117.31% | 169.91% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.near_misses` | records (count) | 10.4000 | 17.9000 | 44.2000 | 72.12% | 146.93% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.queue_events` | records (count) | 50.3000 | 103.2000 | 269.3000 | 105.17% | 160.95% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.region_entries` | records (count) | 5.6000 | 11.7000 | 30.9000 | 108.93% | 164.10% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.region_exits` | records (count) | 5.3000 | 11.5000 | 30.7000 | 116.98% | 166.96% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.spawns` | records (count) | 6.6000 | 12.2000 | 31.6000 | 84.85% | 159.02% | materially_sensitive |
| `pedestrian_route:west_to_south` | `event_counts.violations` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `event_counts.yields` | records (count) | 0.0000 | 0.0000 | 0.0000 | - | - | converged |
| `pedestrian_route:west_to_south` | `maximum_queue_duration_s` | seconds (continuous) | 4.4100 | 4.6550 | 8.6850 | 5.56% | 86.57% | materially_sensitive |
| `pedestrian_route:west_to_south` | `maximum_queue_length_agents` | agents (count) | 1.7000 | 1.9000 | 2.3000 | 11.76% | 21.05% | converged |
| `pedestrian_route:west_to_south` | `mean_control_delay_s` | seconds (continuous) | 12.8792 | 10.5499 | 9.7281 | 18.09% | 7.79% | materially_sensitive |
| `pedestrian_route:west_to_south` | `mean_queue_duration_s` | seconds (continuous) | 0.2927 | 0.2116 | 0.2100 | 27.70% | 0.76% | converged |
| `pedestrian_route:west_to_south` | `mean_stopped_delay_s` | seconds (continuous) | 2.3758 | 1.8110 | 1.8039 | 23.77% | 0.39% | converged |
| `pedestrian_route:west_to_south` | `mean_travel_time_s` | seconds (continuous) | 30.4086 | 29.6119 | 29.2997 | 2.62% | 1.05% | converged |
| `pedestrian_route:west_to_south` | `throughput_agents_per_s` | agents_per_second (continuous) | 0.0347 | 0.0377 | 0.0407 | 8.65% | 7.96% | materially_sensitive |
| `pedestrian_route:west_to_south` | `total_control_delay_s` | seconds (continuous) | 64.4350 | 113.3750 | 293.7350 | 75.95% | 159.08% | materially_sensitive |
| `pedestrian_route:west_to_south` | `total_stopped_delay_s` | seconds (continuous) | 11.0500 | 20.7550 | 56.9200 | 87.83% | 174.25% | materially_sensitive |
| `pedestrian_route:west_to_south` | `total_travel_time_s` | seconds (continuous) | 156.5250 | 332.8750 | 894.9150 | 112.67% | 168.84% | materially_sensitive |

## Counts

| Variant | Metrics | Converged | Materially sensitive | Inconclusive |
| --- | --- | --- | --- | --- |
| `ew_priority` | 254 | 54 | 185 | 15 |
| `ns_priority` | 254 | 50 | 189 | 15 |
