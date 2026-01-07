#!/usr/bin/env python3
import sys
import argparse
import pandas as pd
import numpy as np
import scipy.stats as st

def main():
    parser = argparse.ArgumentParser(description="Analyze Reichtum self-play results")
    parser.add_argument("input_file", help="Path to the CSV output file")
    args = parser.parse_args()

    # Read the CSV file
    # The output format from 'self_play' example seems to be "ScoreA,ScoreB" (no header or header "A(d=X),B(d=Y)")
    # Based on previous runs, it has a header.

    try:
        df = pd.read_csv(args.input_file)
    except Exception as e:
        print(f"Error reading file: {e}")
        sys.exit(1)

    # Assuming columns are like "A(d=X)", "B(d=Y)"
    if len(df.columns) < 2:
        print("Error: Input file must have at least 2 columns")
        sys.exit(1)

    print(f"Analyzed {len(df)} games.")
    print("\nDescriptive Statistics:")
    print(df.describe())

    # Analyze Win Rates
    # Determine winner for each row
    # In Reichtum, higher score wins.
    # Ties are broken by fewest purchased cards, but we don't have that info in the CSV.
    # We will assume score tie = tie for now.

    col1 = df.columns[0]
    col2 = df.columns[1]

    wins1 = (df[col1] > df[col2]).sum()
    wins2 = (df[col2] > df[col1]).sum()
    ties = (df[col1] == df[col2]).sum()

    print(f"\nWin Counts:")
    print(f"{col1}: {wins1} ({wins1/len(df)*100:.1f}%)")
    print(f"{col2}: {wins2} ({wins2/len(df)*100:.1f}%)")
    print(f"Ties: {ties} ({ties/len(df)*100:.1f}%)")

    # Statistical Test (Wilcoxon signed-rank test)
    # Checks if the distribution of differences is symmetric around zero.
    # alternative='greater' checks if col1 > col2

    # Handling ties in Wilcoxon: 'pratt' or 'wilcox' zero_method.
    # scipy.stats.wilcoxon handles ties in differences (where score1 == score2).

    try:
        stat, p_value = st.wilcoxon(df[col1], df[col2], alternative='greater', zero_method='pratt')
        print(f"\nWilcoxon Signed-Rank Test ({col1} > {col2}):")
        print(f"Statistic: {stat}")
        print(f"p-value: {p_value}")

        if p_value < 0.05:
            print(f"Result: {col1} is significantly better than {col2} (p < 0.05)")
        else:
            print(f"Result: No significant difference detected favoring {col1} (p >= 0.05)")

    except ValueError as e:
        print(f"Wilcoxon test failed (maybe all scores are identical?): {e}")

if __name__ == "__main__":
    main()
