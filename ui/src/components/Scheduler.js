import React, { Component } from 'react'
import { Header, Button, Form, Label } from 'semantic-ui-react'
import DatePicker from 'react-datepicker'
import numeral from 'numeral'
import moment from 'moment'

import 'react-datepicker/dist/react-datepicker.css'
import './Scheduler.css'

const dateFormat = {
  sameElse: 'llll'
}

export function Summary({ condition }) {
    if ('block' in condition) {
      const block = parseInt(condition.block.substr(2), 16)
      return (
        <span>at block #{ numeral(block).format() }</span>
      )
    }

    if ('time' in condition) {
      return (
        <span>{ moment.unix(condition.time).calendar(null, dateFormat) }</span>
      )
    }
}

export default class Scheduler extends Component {
  static defaultProps = {
    onNewCondition: () => {},
    currentBlock: 0,
  }

  state = {
    mode: 'time',
    inputBlock: '',
    parseBlock: 0,
    minBlock: this.props.currentBlock,

    startTime: moment(),
    inputTime: moment().add(3, 'hours')
  }

  setModeTime = () => this.setState({ mode: 'time'})
  setModeBlock = () => this.setState({ mode: 'block' })

  isTimeValid () {
    return this.state.inputTime > moment()
  }

  componentDidMount () {
    this.props.onNewCondition({ time: this.state.inputTime.unix() })
  }

  componentWillReceiveProps (newProps) {
    const { condition } = newProps
    if (this.props.condition === condition) {
      return
    }

    if ('time' in condition) {
      this.setState({
        mode: 'time',
        inputTime: moment.unix(condition.time)
      })
    }

    if ('block' in condition) {
      this.setState({
        mode: 'block',
        ...this.parseBlock(condition.block)
      })
    }
  }

  render () {
    const { mode } = this.state
    return (
      <div style={styles.scheduler}>
        <Header as='h3'>I want my transaction to run at specific:</Header>
        <Button.Group attached widths={2}>
          <Button
            active={mode === 'time' }
            onClick={ this.setModeTime }
            primary={ mode === 'time' }
          >time</Button>
          <Button.Or color='purple' />
          <Button
            active={mode === 'block' }
            onClick={ this.setModeBlock }
            primary={ mode === 'block' }
          >block</Button>
        </Button.Group>

        {mode === 'time' ? this.renderTimeSelector() : null }
        {mode === 'block' ? this.renderBlockSelector() : null }

        <div style={{marginTop: '1rem'}}>
          { this.renderSummary() }
        </div>
      </div>
    )
  }

  handleInputTime = inputTime => {
    this.setState({ inputTime })
    if (this.isTimeValid()) {
      this.props.onNewCondition({ time: inputTime.unix() })
    }
  }

  renderTimeSelector () {
    const { inputTime, startTime } = this.state

    return (
      <Form as='div'>
        <Form.Field>
          <DatePicker
            inline
            onChange={ this.handleInputTime }
            selected={ inputTime }
            showTimeSelect
            minDate={ startTime }
          />
          { this.renderTimeHelp() }
        </Form.Field>
      </Form>
    )
  }

  renderTimeHelp () {
    if (!this.isTimeValid()) {
      return (
        <Label pointing basic color='red'>You need to select a future time.</Label>
      )
    }

    return null
  }

  parseBlock (inputBlock) {
    const minBlock = this.props.currentBlock
    const parsedBlock = inputBlock.startsWith('0x')
      ? parseInt(inputBlock.substr(2), 16)
      : numeral(inputBlock).value()
    const validBlock = parsedBlock > minBlock

    return { inputBlock, validBlock, minBlock, parsedBlock }
  }

  handleInputBlock = (ev) => {
    const state = this.parseBlock(ev.target.value)
    this.setState(state)

    if (state.validBlock) {
      this.props.onNewCondition({
        block: '0x' + state.parsedBlock.toString(16)
      })
    }
  }

  renderBlockSelector () {
    const { inputBlock } = this.state
    return (
      <Form as='div'>
        <Form.Field>
          <input
            type='text'
            placeholder={ 'enter block number' }
            value={ inputBlock }
            onChange={ this.handleInputBlock }
          />
          { this.renderBlockHelp() }
        </Form.Field>
      </Form>
    )
  }

  renderBlockHelp () {
    const { minBlock, validBlock } = this.state
    const { currentBlock } = this.props
    if (!validBlock) {
      return (
        <Label pointing basic color='red'>Number needs to be greater than { numeral(minBlock).format() }</Label>
      )
    }

    if (currentBlock) {
      return (
        <Label pointing>Current block: <strong>#{ numeral(currentBlock).format() }</strong></Label>
      )
    }

    return null
  }

  renderSummary () {
    const { parsedBlock, mode, validBlock, inputTime } = this.state

    if (mode === 'block' && validBlock) {
      return (
        <p>Your transaction will be propagated to the network at block #{ numeral(parsedBlock).format() }.</p>
      )
    }

    if (mode === 'time' && this.isTimeValid()) {
      return (
        <p>Your transaction will be propagted to the network { inputTime.calendar(null, dateFormat) } ({ moment(inputTime).fromNow() })</p>
      )
    }
  }
}

const styles = {
  scheduler: {
    width: '100%',
    maxWidth: '350px',
    margin: 'auto',
    textAlign: 'center'
  }
}
