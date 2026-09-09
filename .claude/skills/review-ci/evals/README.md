# review-ci trigger eval

`trigger_eval.json` is a skill-creator trigger eval-set: a list of
`{ "query": ..., "should_trigger": true|false }` cases that check whether the
`review-ci` **description** causes Claude to invoke the skill (positive cases) and
leaves it alone otherwise (negatives, e.g. red-CI queries that belong to
`fix-tests`).

Run it with skill-creator's `run_eval.py` (needs the `claude` CLI; best from an
**interactive** Claude Code session — nested `claude -p` is too slow headless):

```
SC=../../skill-creator   # path to the skill-creator skill
PYTHONPATH=$SC python -m scripts.run_eval \
  --eval-set trigger_eval.json \
  --skill-path .. \
  --runs-per-query 3 --verbose
```

Then feed the results JSON to `scripts/improve_description.py` to propose a
sharper description. See `skills/skill-creator/SKILL.md`
(*Description Optimization*).
