"""
Hashes emails in a CSV and preserves some additional columns, outputting a new CSV.

```
python3 hash_emails.py attendees.csv attendees_hashed.csv --email-column=email --keep-columns="start date,end date"
```
"""

import argparse
import csv
import hashlib
import sys

def sha256_email(email: str) -> str:
    """Normalize and hash an email using SHA-256."""
    normalized_email = email.strip().lower()
    return hashlib.sha256(normalized_email.encode('utf-8')).hexdigest()

def process_csv(input_file: str, output_file: str, email_column: str, keep_columns: list):
    with open(input_file, newline='', encoding='utf-8') as infile:
        reader = csv.DictReader(infile)
        fieldnames = [col for col in keep_columns if col != email_column] + [email_column]

        with open(output_file, 'w', newline='', encoding='utf-8') as outfile:
            writer = csv.DictWriter(outfile, fieldnames=fieldnames)
            writer.writeheader()

            for row in reader:
                if email_column not in row:
                    print(f"Missing email column '{email_column}' in row: {row}", file=sys.stderr)
                    continue

                row[email_column] = sha256_email(row[email_column])
                filtered_row = {col: row[col] for col in fieldnames if col in row}
                writer.writerow(filtered_row)

def main():
    parser = argparse.ArgumentParser(description="Hash emails in a CSV file using SHA-256.")
    parser.add_argument("input_csv", help="Path to input CSV file")
    parser.add_argument("output_csv", help="Path to output CSV file")
    parser.add_argument("--email-column", required=True, help="Name of the column containing email addresses")
    parser.add_argument("--keep-columns", required=True, help="Comma-separated list of columns to keep (including email column)")

    args = parser.parse_args()
    keep_columns = [col.strip() for col in args.keep_columns.split(",")]

    process_csv(args.input_csv, args.output_csv, args.email_column, keep_columns)

if __name__ == "__main__":
    main()
