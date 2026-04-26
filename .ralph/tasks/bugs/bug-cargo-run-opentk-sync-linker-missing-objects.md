## Bug: cargo run opentk-sync can fail linking with missing target object files <status>not_started</status> <passes>false</passes> <priority>medium</priority>

<description>
While executing Story 10 Task 1 on 2026-04-26, the preliminary command
`cargo run -p opentk-db --bin opentk-sync -- --help` failed before any sync work
started. Cargo compiled `opentk-db`, then `cc`/`ld` returned exit status 1
because several expected object files under `target/debug/deps/` were missing.

Observed linker diagnostics included:

```text
/usr/bin/ld: cannot find /home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/target/debug/deps/opentk_sync-a96a3e8afe419d49.1z6fid4ybuoux756c44stl3aa.1ucd7ee.rcgu.o: No such file or directory
/usr/bin/ld: cannot find /home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/target/debug/deps/opentk_sync-a96a3e8afe419d49.29i2zz9atkbw82as8a0jwauef.1ucd7ee.rcgu.o: No such file or directory
/usr/bin/ld: cannot find /home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/target/debug/deps/opentk_sync-a96a3e8afe419d49.72jfhoqne0ozydgvofnyg2p02.1ucd7ee.rcgu.o: No such file or directory
collect2: error: ld returned 1 exit status
error: could not compile `opentk-db` (bin "opentk-sync") due to 1 previous error
```

The repository must determine whether this is caused by concurrent Cargo use,
target directory corruption, workspace build configuration, or another local
build-system issue. Do not hide this as a transient failure without evidence.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [ ] I created a Red unit and/or integration test that captures the bug
- [ ] I made the test green by fixing
- [ ] I manually verified the bug, and created a new Red test if not working still
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
