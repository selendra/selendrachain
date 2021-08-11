#!/bin/bash
{
    echo '[Unit]'
    echo 'Description=Indracore'
    echo '[Service]'
    echo 'Type=exec'
    echo 'WorkingDirectory='`pwd`
    echo 'ExecStart='`pwd`'/target/release/selendra --chain=sel --ws-external --rpc-cors "*"'
    echo '[Install]'
    echo 'WantedBy=multi-user.target'
} > /etc/systemd/system/indracore.service