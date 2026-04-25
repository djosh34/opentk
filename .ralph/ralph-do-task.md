## Progress Log

On startup, read your previous progress
```bash
/bin/bash .ralph/progress_read.sh "<codex>"
```

Append to the progress log — it is your working memory across context windows.
Please write very often.
```bash
/bin/bash .ralph/progress_append.sh "<codex>" << 'EOF_APPEND_PROGRESS_LOG'
- what you did
- what happened
- should do next, after quitting immediately due to context limit
EOF_APPEND_PROGRESS_LOG
```

# Your task
- [ ] read .ralph/current_task.txt
    - [ ] this file will contain a path to a task
    - [ ] from this on we will call that [task name].md
- [ ] read the [task name].md file from the path specified
- [ ] complete the work like this:
    - [ ] If you read nothing/ TO BE VERIFIED, follow these steps:
        - [ ] Create a plan first in separate file in plans subdir .ralph/tasks/[story]/[task]_plans/[plan].md
        - [ ] Make sure this plan uses $improve-code-boundaries skill to have better code and you MUST use $tdd skill to plan and execute
        - [ ] Replace end of plan with 'TO BE VERIFIED' and add path to the plan
        - [ ] Once your happy with plan/interface design/types/enums, replace end of plan with 'NOW EXECUTE'
        - [ ] QUIT IMMEDIATELY!
    - [ ] When you read 'NOW EXECUTE': 
        - [ ] you must read $tdd skill, it is not only about tdd, but also about the whole coding mindset of Red-Green tdd and code quality
        - [ ] execute the plan as written, fix the compile errors, and tick off the boxes when you do them. 
        - [ ] If at any point you find that the design was not correct, and types are in need of change, switch 'NOW EXECUTE' back to 'TO BE VERIFIED'
        - [ ] If switched, QUIT IMMEDIATELY, else continue until checks pass (only if design is still right)

- [ ] you are really done if and only if ALL of these pass 100%:
    - [ ] `make check` & `make lint` (they are the same)
    - [ ] `make test`
    - [ ] You MUST NOT run `make test-long` or any e2e lane for a normal non-story-finishing task
    - [ ] `make test-long` / e2e is allowed only when you are finishing the entire story, or when the task explicitly says the long / e2e lane is required
    - [ ] Treat `make test-long` / e2e as a story-end validation gate only, never as a default end-of-task check
    - [ ] Time is valuable: running the long / e2e lane on every task slows development down far too much and is therefore the wrong default
    - [ ] Final check if code isn't more muddy using improve-code-boundaries skill, otherwise resolve it

- [ ] only when you're done, and all checks pass:
    - [ ] set in [task name].md
        - [ ] set `<passes>true</passes>`
    - [ ] CRUCIAL: run `/bin/bash .ralph/task_switch.sh` to indicate that you want to switch task. 
                This can be when you're done or just when you want to switch (e.g. for going to subtask)
    - [ ] commit with: `task finished [task name]: [insert text]`
        - [ ] include summary of what was done in commit message (evidence for tests completing, challenges faced during
          implementation)
        - [ ] Make sure to add all files, please do not forget commiting any file when completing a task, also the stuff in .ralph
    - [ ] push commits with: `git push`
    - [ ] Write any learnings/surprises to AGENTS.md, only if it is really needed to help future engineers.
      -  Less in AGENTS.md is better than more
    - [ ] QUIT IMMEDIATELY
