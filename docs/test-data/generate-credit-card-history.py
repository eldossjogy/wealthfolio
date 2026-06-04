#!/usr/bin/env python3
"""Generate docs/test-data/credit-card-history.csv (demo data).

Goals baked in:
- Every calendar day Dec 1 2025 -> Jun 3 2026 has at least one expense.
- Mortgage is paid bi-weekly: $1300 every 14 days (usually twice a month).
- Two travel events live in the last two months: the Chicago WEDDING trip
  (May 15-17) and a new Montreal weekend TRIP (May 22-24). A Montreal trip in
  January is kept for history.
- Spending times follow a weekday/hour pattern so the "When you spend" heatmap
  demos well: heavy grocery runs on Mon 6p, Wed 9a, Fri 9a and Sat midday;
  light Tue/Thu; coffee mornings; lunches at noon; dinners in the evening.

Deterministic (seeded) so re-running yields the same file.
"""
import csv
import random
from datetime import date, datetime, timedelta

random.seed(20260604)

START = date(2025, 12, 1)
END = date(2026, 6, 3)

rows = []  # (datetime, activityType, amount, currency, fee, comment)


def add(dt, amount, comment, atype="WITHDRAWAL", fee=0.0):
    rows.append((dt, atype, round(amount, 2), "USD", round(fee, 2), comment))


def amt(lo, hi):
    return round(random.uniform(lo, hi), 2)


def at(d, hour, minute):
    return datetime(d.year, d.month, d.day, hour, minute)


# --- Merchant pools -------------------------------------------------------
COFFEE = ["STARBUCKS STORE 04287", "DUNKIN #348772", "BLUE BOTTLE COFFEE NYC"]
LUNCH = [
    "CHIPOTLE 2241", "SWEETGREEN BROADWAY", "CHICK-FIL-A #02314",
    "SUBWAY #28411", "PANERA BREAD #4892", "SHAKE SHACK MADISON SQ",
]
GROCERY = [
    "WHOLE FOODS MARKET #10142", "TRADER JOE'S #471",
    "COSTCO WHSE #1184", "SAFEWAY #2841 GROCERIES",
]
DINNER = [
    "OLIVE GARDEN ITALIAN REST", "CHEESECAKE FACTORY #112",
    "THE LOCAL BISTRO #18", "MAGGIANO'S LITTLE ITALY",
]
DELIVERY = ["DOORDASH ORDER", "UBER EATS ORDER"]
PHARMACY = ["CVS/PHARMACY #03127", "WALGREENS #04412"]
GAS = ["CHEVRON 0207854", "SHELL OIL 575218400", "COSTCO GAS STATION"]
RIDESHARE = ["UBER TRIP HELP.UBER.COM", "LYFT RIDE"]
RETAIL = [
    "TARGET T-1487", "BEST BUY #00879", "NORDSTROM #0418",
    "LULULEMON ATHLETICA", "SEPHORA #1842", "KOHL'S #0738", "BARNES & NOBLE #2287",
]


def amazon():
    return "AMAZON.COM*%06X" % random.randint(0, 0xFFFFFF)


def order(name):
    return "%s %05d" % (name, random.randint(10000, 99999))


# --- Event blocks (explicit, override the daily template on these days) ---
EVENT_DAYS = set()


def event(d, hour, minute, amount, comment, fee=0.0):
    EVENT_DAYS.add(d)
    add(at(d, hour, minute), amount, comment, fee=fee)


# Chicago WEDDING trip (Apr 17-19) -- last two months
event(date(2026, 4, 17), 5, 30, 42.80, "UBER TRIP TO JFK AIRPORT")
event(date(2026, 4, 17), 7, 0, 574.80, "UNITED AIRLINES JFK-ORD ROUNDTRIP X2")
event(date(2026, 4, 17), 11, 0, 38.50, "LYFT CHICAGO ORD")
event(date(2026, 4, 17), 15, 0, 418.00, "HILTON CHICAGO MAGNIFICENT MILE")
event(date(2026, 4, 17), 20, 0, 142.60, "GIBSONS BAR & STEAKHOUSE CHICAGO")
event(date(2026, 4, 18), 9, 0, 12.80, "INTELLIGENTSIA COFFEE CHICAGO")
event(date(2026, 4, 18), 10, 0, 200.00, "CRATE & BARREL WEDDING REGISTRY GIFT")
event(date(2026, 4, 18), 16, 0, 28.40, "LYFT CHICAGO DOWNTOWN")
event(date(2026, 4, 18), 18, 30, 96.00, "THE ROOKERY WEDDING VENUE BAR TAB")
event(date(2026, 4, 19), 12, 0, 48.50, "LOU MALNATI'S PIZZERIA CHICAGO")
event(date(2026, 4, 19), 21, 0, 46.20, "UBER TRIP FROM JFK")

# Montreal WEEKEND TRIP (May 29-31) -- last two months
event(date(2026, 5, 29), 16, 0, 61.20, "UBER TRIP TO JFK AIRPORT")
event(date(2026, 5, 29), 18, 0, 388.40, "AIR CANADA JFK-YUL")
event(date(2026, 5, 29), 22, 30, 78.40, "L'GROS LUXE MONTREAL QC", 2.35)
event(date(2026, 5, 30), 9, 30, 9.20, "TIM HORTONS MONTREAL YUL", 0.28)
event(date(2026, 5, 30), 11, 0, 512.00, "HOTEL NELLIGAN VIEUX-MONTREAL", 15.36)
event(date(2026, 5, 30), 13, 30, 42.60, "MARCHE JEAN-TALON MONTREAL QC", 1.28)
event(date(2026, 5, 30), 16, 0, 32.00, "MUSEE DES BEAUX-ARTS MONTREAL", 0.96)
event(date(2026, 5, 30), 20, 0, 168.30, "LE FILET RESTAURANT MONTREAL QC", 5.05)
event(date(2026, 5, 31), 10, 0, 13.40, "CAFE OLIMPICO MONTREAL QC", 0.40)
event(date(2026, 5, 31), 13, 0, 41.20, "SCHWARTZ'S DELI MONTREAL QC", 1.24)
event(date(2026, 5, 31), 19, 0, 57.80, "UBER TRIP FROM JFK")


# --- Bi-weekly mortgage: $1300 every 14 days, anchored Fri Dec 5 2025 ------
m = date(2025, 12, 5)
while m <= END:
    add(at(m, 9, 5), 1300.00, "ROCKET MORTGAGE PYMT 0049281")
    m += timedelta(days=14)


# --- Monthly subscriptions & bills ----------------------------------------
# (day_of_month, hour, minute, comment, amount or (lo,hi))
MONTHLY = [
    (1, 2, 5, "NETFLIX.COM", 15.49),
    (2, 2, 10, "SPOTIFY USA", 11.99),
    (2, 2, 15, "DISNEY PLUS", 13.99),
    (4, 3, 15, "APPLE.COM/BILL ICLOUD", 2.99),
    (9, 5, 40, "OPENAI CHATGPT SUBSCRIPTION", 20.00),
    (12, 6, 30, "PLANET FITNESS MEMBER FEES", 24.99),
    (17, 6, 5, "NYTIMES DIGITAL SUBSCRIPTION", 17.00),
    (18, 2, 20, "HBO MAX.COM SUBSCRIPTION", 14.99),
    (20, 4, 25, "ADOBE CREATIVE CLOUD", 22.99),
    (8, 7, 20, "CON EDISON ENERGY BILL", (110.0, 170.0)),
    (16, 7, 35, "NATIONAL GRID GAS UTILITY", (41.0, 50.0)),
    (10, 12, 10, "VERIZON WIRELESS PAYMENT", 85.32),
    (11, 12, 30, "XFINITY COMCAST INTERNET", 79.99),
    (22, 11, 0, "GEICO AUTO INSURANCE", 142.40),
]


def month_iter(start, end):
    y, mth = start.year, start.month
    while (y, mth) <= (end.year, end.month):
        yield y, mth
        mth += 1
        if mth > 12:
            mth = 1
            y += 1


for y, mth in month_iter(START, END):
    for dom, h, mi, comment, a in MONTHLY:
        try:
            d = date(y, mth, dom)
        except ValueError:
            continue
        if not (START <= d <= END):
            continue
        amount = amt(*a) if isinstance(a, tuple) else a
        add(at(d, h, mi), amount, comment)

    # Monthly autopay credit-card payment (TRANSFER_IN) around the 28th.
    try:
        d = date(y, mth, 28)
        if START <= d <= END:
            add(at(d, 6, 0), amt(4000, 5500), "AUTOPAY PAYMENT - THANK YOU",
                atype="TRANSFER_IN")
    except ValueError:
        pass

    # Monthly Amazon refund (CREDIT) mid-month.
    d = date(y, mth, 11)
    if START <= d <= END and random.random() < 0.7:
        add(at(d, 14, 0), amt(25, 40),
            order("AMAZON.COM REFUND ORDER"), atype="CREDIT")


# --- Daily discretionary template (drives the heatmap) --------------------
# weekday: Mon=0 ... Sun=6
day = START
gas_due = START
while day <= END:
    wd = day.weekday()

    if day in EVENT_DAYS:
        day += timedelta(days=1)
        continue

    # Morning coffee almost every day (small).
    if wd == 6:
        add(at(day, 9, 30), amt(14, 28), random.choice(COFFEE))  # Sun brunch coffee
    elif wd == 5:
        add(at(day, 9, 10), amt(6, 9), random.choice(COFFEE))
    else:
        ch = 7 if wd in (0, 2, 4) else (8 if wd == 1 else 8)
        cm = 45 if wd in (0, 2, 4) else (5 if wd == 1 else 20)
        add(at(day, ch, cm), amt(5, 9), random.choice(COFFEE))

    # Weekday lunch.
    if wd <= 4:
        add(at(day, 12, 30 + random.randint(0, 15)), amt(10, 19), random.choice(LUNCH))

    # Evening / midday anchors -- the heavy heatmap cells.
    if wd == 0:        # Monday 6p big grocery
        add(at(day, 18, 0), amt(150, 210), "WHOLE FOODS MARKET #10142")
    elif wd == 1:      # Tuesday light: just a delivery dinner
        add(at(day, 19, 0), amt(30, 48), order(random.choice(DELIVERY)))
    elif wd == 2:      # Wednesday 9a big grocery
        add(at(day, 9, 0), amt(155, 215), "COSTCO WHSE #1184")
    elif wd == 3:      # Thursday light: pharmacy errand
        add(at(day, 17, 50), amt(13, 27), random.choice(PHARMACY))
    elif wd == 4:      # Friday 9a grocery + evening dinner out
        add(at(day, 9, 0), amt(140, 200), "TRADER JOE'S #471")
        add(at(day, 19, 30), amt(45, 75), random.choice(DINNER))
    elif wd == 5:      # Saturday midday big shop + retail + dinner
        add(at(day, 12, 0), amt(175, 220), "COSTCO WHSE #1184")
        if random.random() < 0.6:
            add(at(day, 16, 0), amt(60, 130), random.choice(RETAIL + [amazon()]))
        add(at(day, 19, 45), amt(55, 72), random.choice(DINNER))
    else:              # Sunday leisure lunch + occasional movie
        add(at(day, 12, 30), amt(12, 25), random.choice(LUNCH))
        if random.random() < 0.5:
            add(at(day, 20, 15), amt(29, 36), "AMC THEATRES 6 LIBERTY")

    # Weekly-ish gas fill.
    if day >= gas_due:
        ghour, gmin = random.choice([(8, 10), (17, 40), (12, 50)])
        add(at(day, ghour, gmin), amt(43, 58), random.choice(GAS))
        gas_due = day + timedelta(days=random.randint(6, 8))

    # Occasional evening rideshare and Amazon order for texture.
    if random.random() < 0.18:
        add(at(day, 21, 10), amt(18, 39), random.choice(RIDESHARE))
    if random.random() < 0.15:
        add(at(day, 20, 35), amt(40, 80), amazon())

    day += timedelta(days=1)


# --- Safety net: guarantee at least one expense every single day ----------
spend_days = {dt.date() for dt, atype, *_ in rows if atype == "WITHDRAWAL"}
d = START
while d <= END:
    if d not in spend_days:
        add(at(d, 12, 0), amt(8, 16), random.choice(LUNCH))
    d += timedelta(days=1)


# --- Write ----------------------------------------------------------------
rows.sort(key=lambda r: r[0])
out = "/Users/aziz/Workspace/wealthfolio-project/wealthfolio-app/wealthfolio/docs/test-data/credit-card-history.csv"
with open(out, "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["date", "activityType", "amount", "currency", "fee", "comment"])
    for dt, atype, amount, currency, fee, comment in rows:
        w.writerow([dt.strftime("%Y-%m-%dT%H:%M:%S"), atype,
                    "%.2f" % amount, currency, "%.2f" % fee, comment])

print("wrote %d rows to %s" % (len(rows), out))
