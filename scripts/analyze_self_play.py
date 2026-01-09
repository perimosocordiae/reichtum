#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "numpy",
#     "pandas",
#     "scipy",
#     "tabulate",
# ]
# ///
import sys
import argparse
import numpy as np
import pandas as pd
import scipy.stats as st


def main():
    parser = argparse.ArgumentParser(description="Analyze Reichtum self-play results")
    parser.add_argument("input_file", help="Path to the CSV output file")
    args = parser.parse_args()

    # Read the CSV file
    try:
        df = pd.read_csv(args.input_file if args.input_file != "-" else sys.stdin)
    except Exception as e:
        print(f"Error reading file: {e}")
        sys.exit(1)

    if len(df.columns) < 2:
        print("Error: Input file must have at least 2 columns")
        sys.exit(1)

    # Print statistics
    print("# Descriptive Statistics\n")
    print(df.describe().to_markdown())

    if len(df.columns) > 2:
        print("\nSkipping paired analysis: more than 2 players found.")
        return

    # Paired Analysis
    col1 = df.columns[0]
    col2 = df.columns[1]
    deltas = df[col1] - df[col2]

    # Ties are broken by fewest purchased cards, but we don't have that info
    # in the CSV. Assume same score is a tie for now.
    wins = np.array([(deltas > 0).sum(), (deltas < 0).sum(), (deltas == 0).sum()])
    wins_df = pd.DataFrame({"Wins": wins}, index=[col1, col2, "Ties"])
    wins_df["Win Rate"] = (wins_df["Wins"] / len(df)).apply(lambda x: f"{x:.2%}")
    print("\n# Win Counts\n")
    print(wins_df.to_markdown())

    # Statistical Test (Wilcoxon signed-rank test)
    # Checks if the distribution of differences is symmetric around zero.
    _, p_12 = st.wilcoxon(deltas, alternative="greater", zero_method="pratt")
    p_21 = 1 - p_12
    print("\n# Wilcoxon Signed-Rank Test\n")
    if p_12 < 0.05:
        print(f"{col1} is significantly better than {col2} (p < 0.05)")
    elif p_21 < 0.05:
        print(f"{col2} is significantly better than {col1} (p < 0.05)")
    else:
        print(f"No significant difference between {col1} and {col2}")
    print(f" - p-value {col1} > {col2}: {p_12}")
    print(f" - p-value {col1} < {col2}: {p_21}")


if __name__ == "__main__":
    main()
