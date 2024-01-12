#!/usr/bin/env python3

import csv

from argparse import ArgumentParser
from datetime import datetime
from pathlib import Path
from typing import NamedTuple

import pandas as pd
import plotly.io as pio
import plotly.graph_objects as go
import plotly.express as px

class Args(NamedTuple):
    report: Path
    apply_msvc_filter: bool


def parse_args() -> Args:
    p = ArgumentParser()
    p.add_argument('--report', required=True, metavar='CSV', type=Path),
    p.add_argument('--apply-msvc-filter', action='store_true')
    a = Args(**p.parse_args().__dict__)
    if not a.report.is_file():
        p.error(f'--report "{a.report}": no such file')
    return a


def main(args: Args) -> None:
    print("csv")
    df_data, sum_data = gather_data_from_csv(args.report)
    print("pandas")
    main_df, sum_df, merged_df = lists_to_dataframes(df_data, sum_data)

    # uncomment this if running the script in vscode notebook
    # pio.renderers.default = "vscode"

    print("plot")
    plot("someheader", main_df, sum_df, merged_df)


def gather_data_from_csv(report: Path) -> tuple[list, list]:
    df_data = []
    sum_data = []
    with report.open('r') as file:
        file.readline() # skip header
        reader = csv.reader(file, delimiter='\t')
        iprev = 1
        tsprev = None
        total_iteration_memory = 0
        for i, timestamp, pid, name, memory, cmdline in reader:
            i = int(i)
            timestamp = int(timestamp)
            pid = pid
            memory = float(memory)
            timestamp = datetime.fromtimestamp(timestamp / 1000)

            if not tsprev:
                tsprev = timestamp

            if i == iprev:
                total_iteration_memory += memory
            else:
                sum_data.append([tsprev, total_iteration_memory])
                total_iteration_memory = 0
                iprev = i
                tsprev = timestamp

            if memory >= 100:
                df_data.append([timestamp, pid, name, memory, cmdline[:100]])
        else:
            sum_data.append([timestamp, total_iteration_memory])
    return df_data, sum_data


def lists_to_dataframes(df_data: list, sum_data: list) -> tuple[pd.DataFrame, pd.DataFrame, pd.DataFrame]:
    main_df = pd.DataFrame(
        columns=['Timestamp', 'PID', 'Name', 'Memory', 'Cmdline'],
        data=df_data,
    )
    sum_df = pd.DataFrame(
        columns=['Timestamp','SumMemory'],
        data=sum_data,
    )
    filtered_df = main_df[main_df['Memory'] > 1]
    max_memory_df = filtered_df.groupby('PID')['Memory'].max().reset_index()
    merged_df = pd.merge(
        max_memory_df, main_df, on=['PID', 'Memory'], how='inner'
    )

    return main_df, sum_df, merged_df


def plot(header: str, main_df: pd.DataFrame, sum_df: pd.DataFrame, merged_df: pd.DataFrame) -> None:
    scatter = px.scatter(
        main_df,
        x="Timestamp",
        y="Memory",
        color="PID",
        hover_data=["Name", "Cmdline"],
    )
    line = px.line(
        sum_df,
        x="Timestamp",
        y="SumMemory",
    )

    # Create a combined figure
    combined_fig = go.Figure()
    for trace in [*scatter.data, *line.data]:
        combined_fig.add_trace(trace)

    combined_fig.update_xaxes(
        dtick=30 * 1000
    )
    combined_fig.update_yaxes(
        type="log",
        tickvals=[
            100, 200, 300, 400,
            1_000, 2_000, 3_000, 4_000,
            10_000, 20_000, 30_000, 40_000,
        ],
    )
    combined_fig.update_layout(
        title=f'Memory Consumption Over Time -- {header}',
        # margin=dict(l=0, r=0, t=0, b=0),
        margin=dict(l=10, r=10, t=50, b=10),
        width=1280,
        height=720,
        xaxis_title="Timestamp",
        yaxis_title="Memory (MB)",
        showlegend=False,
    )
    # combined_fig.update_coloraxes(colorbar_title_text='PID')

    combined_fig.show()

    fig = px.bar(
        merged_df,
        x='PID',
        y='Memory',
        color='Name',
        hover_data=['Name', 'Cmdline'],
        title='Max Memory Consumption Histogram',
    )
    fig.show()


def split_process(pd: str) -> list:
    pid, pname, cmd, pmemory = pd.split(',')
    filename = ''
    for x in cmd.split():
        if '/Fo' in x or '/out' in x:
            filename = x
            break
    return [str(pid), pname, filename, float(pmemory)]


if __name__ == '__main__':
    main(parse_args())
