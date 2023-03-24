# A dead-simple CLI that automates development processes in this repository,
# that are easily expressible via nushell scripts.
def main [] {}

def "main test" [...args: string] {
    cd (repo)
    let args = ([test '--'] | append $args)
    RUST_LOG="debug" cargo $args
}

def-env repo [] {
    git rev-parse --show-toplevel | str trim
}
