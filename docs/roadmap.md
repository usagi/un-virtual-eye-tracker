# UNVET Implementation Roadmap

## Phase alpha

- [x] alpha-4 docs: define architecture and module boundaries
- [x] alpha-4 chore: scaffold workspace crates

## Phase beta

- [x] beta-1 chore: initialize workspace and base crates
- [x] beta-1 feat(core): add config loading
- [x] beta-1 feat(core): add structured logging
- [x] beta-1 feat(core): add unified error handling

## Next queue

- [x] beta-2 feat(input): implement iFacialMocap UDP receiver
- [x] beta-2 feat(input): add receiver state management
- [x] beta-2 test(input): add UDP frame parsing tests

- [ ] beta-3 feat(input): implement iFacialMocap TCP receiver
- [ ] beta-3 feat(input): add TCP frame reassembly
- [ ] beta-3 test(input): add TCP stream parsing tests

## Commit Unit Convention

- Prefix with phase id: `alpha-4`, `beta-1`, `gamma-2`
- Keep one functional objective per commit
